//! VCD 2.0 Playback Control (PBC) interactive state machine.
//!
//! Executes the state transitions defined in `PSD.VCD` and `LOT.VCD`, coordinating
//! still image menus (/SEGMENT/ITEMxxxx.DAT), motion menus, numerical track selections,
//! and standard home video player navigation (PBC, Prev, Next, Return, Default).

use super::lot::LotTable;
use super::psd::{PlayListDesc, PsdDescriptor, PsdTable, SelectionListDesc};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PbcAction {
    /// Play a standard MPEG audio/video track from MPEGAV.
    PlayTrack {
        track_number: u8,
        item_id: u16,
        ptime: u16,
        wtime: u8,
    },
    /// Display a still picture menu from /SEGMENT/ITEMxxxx.DAT.
    DisplayStillMenu {
        item_id: u16,
        segment_index: u16,
        nos: u8,
        bsn: u8,
        timeout_sec: u8,
    },
    /// Play an interactive motion video menu.
    PlayMotionMenu {
        track_number: u8,
        item_id: u16,
        nos: u8,
        bsn: u8,
        timeout_sec: u8,
    },
    /// Stop playback and signal end of disc or sequence.
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PbcState {
    Uninitialized,
    /// Currently waiting on an interactive SelectionList menu
    InSelection {
        desc: SelectionListDesc,
        remaining_timeout: Option<f32>,
    },
    /// Currently playing a PlayList item
    InPlayList {
        desc: PlayListDesc,
        current_item_index: usize,
    },
    /// Waiting after a track for `wtime` seconds
    WaitingDelay {
        desc: PlayListDesc,
        remaining_wait: f32,
    },
    Ended,
}

pub struct PbcEngine {
    pub lot: LotTable,
    pub psd: PsdTable,
    pub current_desc_offset: Option<usize>,
    pub root_menu_offset: Option<usize>,
    pub state: PbcState,
}

impl PbcEngine {
    /// Creates a new PBC engine instance with parsed LOT and PSD tables.
    pub fn new(lot: LotTable, psd: PsdTable) -> Self {
        let root_menu_offset = psd.first_selection_list().map(|s| s.byte_offset);
        Self {
            lot,
            psd,
            current_desc_offset: None,
            root_menu_offset,
            state: PbcState::Uninitialized,
        }
    }

    /// Starts PBC playback according to White Book VCD 2.0 rules (initiating from LID 1).
    pub fn start(&mut self) -> Option<PbcAction> {
        // LID 1 is standard starting entry point
        if let Some(byte_ofs) = self.lot.get_byte_offset(1) {
            return self.execute_descriptor_at_byte_offset(byte_ofs);
        }
        // Fallback to first available descriptor if LID 1 is missing
        if let Some(first_desc) = self.psd.descriptors.first() {
            let ofs = first_desc.byte_offset();
            return self.execute_descriptor_at_byte_offset(ofs);
        }
        None
    }

    /// User presses the dedicated "PBC" button.
    /// Jumps directly to the root/main selection menu if available, or restarts at LID 1.
    pub fn press_pbc(&mut self) -> Option<PbcAction> {
        if let Some(root_ofs) = self.root_menu_offset {
            self.execute_descriptor_at_byte_offset(root_ofs)
        } else {
            self.start()
        }
    }

    /// User enters a number (e.g. 1..=nos) on the numeric keypad or remote control.
    pub fn select_number(&mut self, num: usize) -> Result<Option<PbcAction>, String> {
        match &self.state {
            PbcState::InSelection { desc, .. } => {
                let bsn = desc.bsn as usize;
                let nos = desc.nos as usize;
                if num >= bsn && num < bsn + nos {
                    let sel_index = num - bsn;
                    if sel_index < desc.selections.len() {
                        let target_unit_ofs = desc.selections[sel_index];
                        if target_unit_ofs != 0xFFFF {
                            return Ok(self.execute_descriptor_at_unit_offset(target_unit_ofs));
                        }
                    }
                }
                Ok(None)
            }
            _ => {
                if num == 0 {
                    return Ok(None);
                }
                // When not in a selection list (e.g. currently playing a track),
                // the entered number represents 1-based track index.
                // In VCD White Book, MPEG audio/video tracks are numbered starting from 2.
                let physical_track = (num + 1) as u8;
                if let Some(act) = self.select_track(physical_track) {
                    return Ok(Some(act));
                }
                if let Some(act) = self.select_track(num as u8) {
                    return Ok(Some(act));
                }
                Ok(None)
            }
        }
    }

    /// User selects a specific physical track number (e.g. 2 for track 1).
    /// Searches PSD for a PlayList descriptor whose first item matches `track_number`.
    pub fn select_track(&mut self, track_number: u8) -> Option<PbcAction> {
        let item_id = track_number as u16;
        for desc in &self.psd.descriptors {
            if let PsdDescriptor::PlayList(pl) = desc {
                if pl.items.first().copied() == Some(item_id) {
                    return self.execute_descriptor_at_byte_offset(pl.byte_offset);
                }
            }
        }
        None
    }

    /// User presses "Prev" (Previous track / Previous menu page).
    pub fn press_prev(&mut self) -> Option<PbcAction> {
        let prev_ofs = match &self.state {
            PbcState::InSelection { desc, .. } => desc.prev_ofs,
            PbcState::InPlayList { desc, .. } => desc.prev_ofs,
            PbcState::WaitingDelay { desc, .. } => desc.prev_ofs,
            _ => 0xFFFF,
        };

        if prev_ofs != 0xFFFF {
            self.execute_descriptor_at_unit_offset(prev_ofs)
        } else {
            None
        }
    }

    /// User presses "Next" (Next track / Next menu page).
    pub fn press_next(&mut self) -> Option<PbcAction> {
        let next_ofs = match &self.state {
            PbcState::InSelection { desc, .. } => desc.next_ofs,
            PbcState::InPlayList { desc, .. } => desc.next_ofs,
            PbcState::WaitingDelay { desc, .. } => desc.next_ofs,
            _ => 0xFFFF,
        };

        if next_ofs != 0xFFFF {
            self.execute_descriptor_at_unit_offset(next_ofs)
        } else {
            None
        }
    }

    /// User presses "Return" (Return to parent menu).
    pub fn press_return(&mut self) -> Option<PbcAction> {
        let ret_ofs = match &self.state {
            PbcState::InSelection { desc, .. } => desc.return_ofs,
            PbcState::InPlayList { desc, .. } => desc.return_ofs,
            PbcState::WaitingDelay { desc, .. } => desc.return_ofs,
            _ => 0xFFFF,
        };

        if ret_ofs != 0xFFFF {
            self.execute_descriptor_at_unit_offset(ret_ofs)
        } else if let Some(root_ofs) = self.root_menu_offset {
            self.execute_descriptor_at_byte_offset(root_ofs)
        } else {
            None
        }
    }

    /// User presses "Default" / Enter.
    pub fn press_default(&mut self) -> Option<PbcAction> {
        if let PbcState::InSelection { desc, .. } = &self.state {
            let def_ofs = desc.default_ofs;
            if def_ofs != 0xFFFF {
                return self.execute_descriptor_at_unit_offset(def_ofs);
            }
        }
        None
    }

    /// Called when the currently active media item finishes playing.
    pub fn on_item_finished(&mut self) -> Option<PbcAction> {
        match &self.state {
            PbcState::InPlayList {
                desc,
                current_item_index,
            } => {
                let desc_clone = desc.clone();
                let next_idx = current_item_index + 1;
                if next_idx < desc_clone.items.len() {
                    // Play next item in this playlist
                    let item_id = desc_clone.items[next_idx];
                    self.state = PbcState::InPlayList {
                        desc: desc_clone.clone(),
                        current_item_index: next_idx,
                    };
                    if item_id < 1000 {
                        Some(PbcAction::PlayTrack {
                            track_number: item_id as u8,
                            item_id,
                            ptime: desc_clone.ptime,
                            wtime: desc_clone.wtime,
                        })
                    } else {
                        Some(PbcAction::DisplayStillMenu {
                            item_id,
                            segment_index: item_id - 999,
                            nos: 0,
                            bsn: 0,
                            timeout_sec: 0,
                        })
                    }
                } else {
                    // Playlist finished: check wait time (wtime)
                    if desc_clone.wtime > 0 && desc_clone.wtime < 255 {
                        self.state = PbcState::WaitingDelay {
                            desc: desc_clone.clone(),
                            remaining_wait: desc_clone.wtime as f32,
                        };
                        None
                    } else if desc_clone.wtime == 255 {
                        // Infinite wait until user presses Next/Prev/Return
                        None
                    } else {
                        // Immediate transition to next descriptor
                        if desc_clone.next_ofs != 0xFFFF {
                            self.execute_descriptor_at_unit_offset(desc_clone.next_ofs)
                        } else if let Some(root_ofs) = self.root_menu_offset {
                            self.execute_descriptor_at_byte_offset(root_ofs)
                        } else {
                            self.state = PbcState::Ended;
                            Some(PbcAction::End)
                        }
                    }
                }
            }
            PbcState::InSelection { desc, .. } => {
                let desc_clone = desc.clone();
                // Motion menu finished playing: check loop / timeout
                if desc_clone.timeout_ofs != 0xFFFF && desc_clone.timeout_time > 0 {
                    self.execute_descriptor_at_unit_offset(desc_clone.timeout_ofs)
                } else {
                    // Loop motion menu
                    self.execute_descriptor(PsdDescriptor::SelectionList(desc_clone))
                }
            }
            _ => None,
        }
    }

    /// Clock tick update for delay countdowns and menu timeouts.
    pub fn tick(&mut self, dt_secs: f32) -> Option<PbcAction> {
        match &mut self.state {
            PbcState::WaitingDelay {
                desc,
                remaining_wait,
            } => {
                *remaining_wait -= dt_secs;
                if *remaining_wait <= 0.0 {
                    let next_ofs = desc.next_ofs;
                    if next_ofs != 0xFFFF {
                        self.execute_descriptor_at_unit_offset(next_ofs)
                    } else if let Some(root_ofs) = self.root_menu_offset {
                        self.execute_descriptor_at_byte_offset(root_ofs)
                    } else {
                        self.state = PbcState::Ended;
                        Some(PbcAction::End)
                    }
                } else {
                    None
                }
            }
            PbcState::InSelection {
                desc,
                remaining_timeout,
            } => {
                if let Some(timer) = remaining_timeout {
                    *timer -= dt_secs;
                    if *timer <= 0.0 {
                        let to_ofs = desc.timeout_ofs;
                        if to_ofs != 0xFFFF {
                            return self.execute_descriptor_at_unit_offset(to_ofs);
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Transitions to descriptor located at unit offset (multiplied by `offset_mult`).
    pub fn execute_descriptor_at_unit_offset(&mut self, unit_ofs: u16) -> Option<PbcAction> {
        if unit_ofs == 0xFFFF {
            return None;
        }
        let byte_ofs = (unit_ofs as usize) * self.psd.offset_mult;
        self.execute_descriptor_at_byte_offset(byte_ofs)
    }

    /// Transitions to descriptor located at byte offset in `PSD.VCD`.
    pub fn execute_descriptor_at_byte_offset(&mut self, byte_ofs: usize) -> Option<PbcAction> {
        if let Some(desc) = self.psd.get_by_byte_offset(byte_ofs).cloned() {
            self.current_desc_offset = Some(byte_ofs);
            self.execute_descriptor(desc)
        } else {
            None
        }
    }

    /// Executes a given descriptor, updating state and returning the resulting action.
    fn execute_descriptor(&mut self, desc: PsdDescriptor) -> Option<PbcAction> {
        match desc {
            PsdDescriptor::PlayList(p) => {
                if p.items.is_empty() {
                    if p.next_ofs != 0xFFFF {
                        return self.execute_descriptor_at_unit_offset(p.next_ofs);
                    } else {
                        self.state = PbcState::Ended;
                        return Some(PbcAction::End);
                    }
                }

                let first_item = p.items[0];
                self.state = PbcState::InPlayList {
                    desc: p.clone(),
                    current_item_index: 0,
                };

                if first_item < 1000 {
                    Some(PbcAction::PlayTrack {
                        track_number: first_item as u8,
                        item_id: first_item,
                        ptime: p.ptime,
                        wtime: p.wtime,
                    })
                } else {
                    Some(PbcAction::DisplayStillMenu {
                        item_id: first_item,
                        segment_index: first_item - 999,
                        nos: 0,
                        bsn: 0,
                        timeout_sec: 0,
                    })
                }
            }
            PsdDescriptor::SelectionList(s) => {
                let timeout = if s.timeout_time > 0 {
                    Some(s.timeout_time as f32)
                } else {
                    None
                };

                let item_id = s.item_id;
                let nos = s.nos;
                let bsn = s.bsn;
                let timeout_time = s.timeout_time;

                self.state = PbcState::InSelection {
                    desc: s,
                    remaining_timeout: timeout,
                };

                if item_id >= 1000 {
                    Some(PbcAction::DisplayStillMenu {
                        item_id,
                        segment_index: item_id - 999,
                        nos,
                        bsn,
                        timeout_sec: timeout_time,
                    })
                } else if item_id >= 2 && item_id <= 99 {
                    Some(PbcAction::PlayMotionMenu {
                        track_number: item_id as u8,
                        item_id,
                        nos,
                        bsn,
                        timeout_sec: timeout_time,
                    })
                } else {
                    Some(PbcAction::DisplayStillMenu {
                        item_id,
                        segment_index: 1,
                        nos,
                        bsn,
                        timeout_sec: timeout_time,
                    })
                }
            }
            PsdDescriptor::EndList(_) => {
                self.state = PbcState::Ended;
                Some(PbcAction::End)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pbc_flow_synthetic() {
        let mut raw_lot = vec![0u8; 16];
        raw_lot[2] = 0; raw_lot[3] = 0; // LID 1 -> unit 0
        raw_lot[4] = 0; raw_lot[5] = 2; // LID 2 -> unit 2
        raw_lot[6] = 0; raw_lot[7] = 5; // LID 3 -> unit 5

        let lot = LotTable::parse(&raw_lot, 8).unwrap();

        let mut raw_psd = Vec::new();
        // PlayList LID 1 at unit 0 (byte 0)
        raw_psd.push(0x10);
        raw_psd.push(1); // noi = 1
        raw_psd.extend_from_slice(&1u16.to_be_bytes()); // lid = 1
        raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // prev
        raw_psd.extend_from_slice(&2u16.to_be_bytes()); // next = unit 2
        raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // ret
        raw_psd.extend_from_slice(&0u16.to_be_bytes()); // ptime
        raw_psd.push(0); // wtime
        raw_psd.push(0); // atime
        raw_psd.extend_from_slice(&2u16.to_be_bytes()); // items[0] = track 2

        // SelectionList LID 2 at unit 2 (byte 16)
        raw_psd.push(0x18);
        raw_psd.push(0); // flags
        raw_psd.push(2); // nos = 2
        raw_psd.push(1); // bsn = 1
        raw_psd.extend_from_slice(&2u16.to_be_bytes()); // lid = 2
        raw_psd.extend_from_slice(&0u16.to_be_bytes()); // prev
        raw_psd.extend_from_slice(&5u16.to_be_bytes()); // next = unit 5
        raw_psd.extend_from_slice(&0u16.to_be_bytes()); // ret
        raw_psd.extend_from_slice(&0xFFFFu16.to_be_bytes()); // def
        raw_psd.extend_from_slice(&5u16.to_be_bytes()); // to = unit 5
        raw_psd.push(10); // timeout = 10s
        raw_psd.push(1); // loop
        raw_psd.extend_from_slice(&1000u16.to_be_bytes()); // item_id = 1000 (/SEGMENT/ITEM0001.DAT)
        raw_psd.extend_from_slice(&5u16.to_be_bytes()); // sel 1 -> unit 5
        raw_psd.extend_from_slice(&5u16.to_be_bytes()); // sel 2 -> unit 5

        // EndList at unit 5 (byte 40)
        raw_psd.push(0x1F);
        raw_psd.push(0);
        raw_psd.extend_from_slice(&0u16.to_be_bytes());
        raw_psd.extend_from_slice(&[0, 0, 0, 0]);

        let psd = PsdTable::parse(&raw_psd, 8).unwrap();
        let mut engine = PbcEngine::new(lot, psd);

        // Start PBC -> plays track 2
        let act1 = engine.start().expect("start action");
        assert_eq!(
            act1,
            PbcAction::PlayTrack {
                track_number: 2,
                item_id: 2,
                ptime: 0,
                wtime: 0,
            }
        );

        // Track finished -> transitions to unit 2 (SelectionList)
        let act2 = engine.on_item_finished().expect("next action");
        assert_eq!(
            act2,
            PbcAction::DisplayStillMenu {
                item_id: 1000,
                segment_index: 1,
                nos: 2,
                bsn: 1,
                timeout_sec: 10,
            }
        );

        // User selects 1 -> unit 5 (EndList)
        let act3 = engine.select_number(1).unwrap().expect("selection action");
        assert_eq!(act3, PbcAction::End);
        assert_eq!(engine.state, PbcState::Ended);

        // Press PBC -> restarts at root menu (SelectionList)
        let act4 = engine.press_pbc().expect("pbc action");
        assert_eq!(
            act4,
            PbcAction::DisplayStillMenu {
                item_id: 1000,
                segment_index: 1,
                nos: 2,
                bsn: 1,
                timeout_sec: 10,
            }
        );
    }
}
