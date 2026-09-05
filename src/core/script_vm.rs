//! VCDSCRIPT Coroutine Virtual Machine implementation.

use crate::core::script_ast::{CondOp, Expr, ScriptProgram, Statement};
use std::ops::Bound::Excluded;
use std::ops::Bound::Unbounded;

pub trait VmHost {
    fn draw_image(&mut self, filename: &str, x: i32, y: i32, mode: i32);
    fn draw_cursor(&mut self, x: i32, y: i32);
    fn play_sound(&mut self, filename: &str);
    fn play_video(&mut self, filename: &str, start_frame: i32, end_frame: i32, exit_page: Option<&str>);
    fn karaoke_set(&mut self, channel: i32, mode: i32) {
        let _ = (channel, mode);
    }
    fn karaoke_get(&self, _index: i32) -> i32 {
        -1
    }
    fn karaoke_del(&mut self, _index: i32) {}
    fn karaoke_ins(&mut self, _index: i32, _val: i32) {}
    fn karaoke_play(&mut self) -> bool {
        false
    }
    fn get_time_ms(&self) -> u64;
    /// Returns time in 0.1-second (100ms) units, exactly matching original
    /// AUTORUN.EXE (0x40d2b7: CRT time_t * 10 % 65535, i.e. 1 unit = 100ms).
    fn get_time_units(&self) -> i32 {
        (self.get_time_ms() / 100) as i32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnrecognizedInstruction {
    pub line_no: u32,
    pub raw_code: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmState {
    Ready,
    Running,
    WaitingForKey { target_var: u8 },
    WaitingForDelay { until_time: i32 },
    WaitingForVideo,
    Finished,
    PausedForAlert(UnrecognizedInstruction),
    Error(String),
}

#[derive(Debug, Clone)]
struct ForLoopState {
    var: u8,
    end: i32,
    loop_start_pc: Option<(u32, usize)>,
}

pub struct VcdScriptVm {
    pub program: ScriptProgram,
    pub variables: [i32; 26],
    pub pc: Option<(u32, usize)>,
    pub state: VmState,
    pub call_stack: Vec<Option<(u32, usize)>>,
    for_stack: Vec<ForLoopState>,
    pub prng_seed: u32,
}

impl VcdScriptVm {
    pub fn new() -> Self {
        Self {
            program: ScriptProgram::default(),
            variables: [0; 26],
            pc: None,
            state: VmState::Ready,
            call_stack: Vec::new(),
            for_stack: Vec::new(),
            prng_seed: 0,
        }
    }

    pub fn load_program(&mut self, program: ScriptProgram) {
        self.program = program;
        self.variables = [0; 26];
        self.call_stack.clear();
        self.for_stack.clear();
        self.pc = self.program.lines.keys().next().map(|&first| (first, 0));
        self.state = if self.pc.is_some() {
            VmState::Ready
        } else {
            VmState::Finished
        };
    }

    pub fn start_at_line(&mut self, line_no: u32) {
        if self.program.lines.contains_key(&line_no) {
            self.pc = Some((line_no, 0));
            self.state = VmState::Running;
        } else {
            // Find the closest line >= line_no
            let next_line = self
                .program
                .lines
                .range(line_no..)
                .next()
                .map(|(&l, _)| (l, 0));
            if let Some(pc) = next_line {
                self.pc = Some(pc);
                self.state = VmState::Running;
            } else {
                self.pc = None;
                self.state = VmState::Finished;
            }
        }
    }

    pub fn get_variable(&self, var: u8) -> i32 {
        let idx = (var.to_ascii_uppercase() as usize).saturating_sub('A' as usize);
        if idx < 26 {
            self.variables[idx]
        } else {
            0
        }
    }

    pub fn set_variable(&mut self, var: u8, val: i32) {
        let idx = (var.to_ascii_uppercase() as usize).saturating_sub('A' as usize);
        if idx < 26 {
            self.variables[idx] = val;
        }
    }

    /// Computes the default next PC in sequential order.
    fn compute_next_pc(&self, line_no: u32, stmt_idx: usize) -> Option<(u32, usize)> {
        if let Some(stmts) = self.program.lines.get(&line_no) {
            if stmt_idx + 1 < stmts.len() {
                return Some((line_no, stmt_idx + 1));
            }
        }

        // Advance to next line
        self.program
            .lines
            .range((Excluded(line_no), Unbounded))
            .next()
            .map(|(&next_line, _)| (next_line, 0))
    }

    /// Executes a single statement step.
    pub fn step(&mut self, host: &mut dyn VmHost) -> VmState {
        let Some((line_no, stmt_idx)) = self.pc else {
            self.state = VmState::Finished;
            return VmState::Finished;
        };

        let Some(stmts) = self.program.lines.get(&line_no) else {
            self.pc = None;
            self.state = VmState::Finished;
            return VmState::Finished;
        };

        if stmt_idx >= stmts.len() {
            self.pc = self.compute_next_pc(line_no, stmt_idx);
            return VmState::Running;
        }

        let stmt = stmts[stmt_idx].clone();
        let next_pc = self.compute_next_pc(line_no, stmt_idx);

        match stmt {
            Statement::Assign(var, expr) => {
                let val = expr.eval(&self.variables);
                self.set_variable(var, val);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::CallIrkey(target_var) => {
                self.pc = next_pc;
                self.state = VmState::WaitingForKey { target_var };
            }
            Statement::CallTime(target_var) => {
                let time_val = host.get_time_units();
                self.set_variable(target_var, time_val);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::CallRand(target_var) => {
                if self.prng_seed == 0 {
                    let t = (host.get_time_units() as u32) & 0xff;
                    self.prng_seed = if t == 0 { 1 } else { t };
                }
                // Park-Miller Minimal Standard PRNG (AUTORUN.EXE 0x40cca0: A=16807, M=2147483647)
                let next = ((self.prng_seed as u64 * 16807) % 2147483647) as u32;
                self.prng_seed = next;
                self.set_variable(target_var, next as i32);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::DrawCursor(x_expr, y_expr) => {
                let x = x_expr.eval(&self.variables);
                let y = y_expr.eval(&self.variables);
                host.draw_cursor(x, y);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::DrawImage { file, x, y, mode } => {
                let x_val = x.eval(&self.variables);
                let y_val = y.eval(&self.variables);
                let mode_val = mode.eval(&self.variables);
                host.draw_image(&file, x_val, y_val, mode_val);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::PlaySound(file) => {
                host.play_sound(&file);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::PlayVideo {
                file,
                start_frame,
                end_frame,
                exit_page,
            } => {
                let start = start_frame.eval(&self.variables);
                let end = end_frame.eval(&self.variables);
                host.play_video(&file, start, end, exit_page.as_deref());
                self.pc = next_pc;
                self.state = VmState::WaitingForVideo;
            }
            Statement::KaraokeSet(idx_expr, val_expr) => {
                let idx = idx_expr.eval(&self.variables);
                let val = val_expr.eval(&self.variables);
                host.karaoke_set(idx, val);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::KaraokeGet { index, target_var } => {
                let idx = index.eval(&self.variables);
                let val = host.karaoke_get(idx);
                self.set_variable(target_var, val);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::KaraokeDel(expr) => {
                let idx = expr.eval(&self.variables);
                host.karaoke_del(idx);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::KaraokeIns(idx_expr, val_expr) => {
                let idx = idx_expr.eval(&self.variables);
                let val = val_expr.eval(&self.variables);
                host.karaoke_ins(idx, val);
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::KaraokePlay => {
                let started = host.karaoke_play();
                self.pc = next_pc;
                if started {
                    self.state = VmState::WaitingForVideo;
                } else {
                    self.state = VmState::Running;
                }
            }
            Statement::Goto(expr) => {
                let target_line = expr.eval(&self.variables) as u32;
                if self.program.lines.contains_key(&target_line) {
                    self.pc = Some((target_line, 0));
                    self.state = VmState::Running;
                } else {
                    let err = format!("GOTO 目标行号不存在: {}", target_line);
                    self.state = VmState::Error(err);
                }
            }
            Statement::Gosub(expr) => {
                let target_line = expr.eval(&self.variables) as u32;
                if self.program.lines.contains_key(&target_line) {
                    self.call_stack.push(next_pc);
                    self.pc = Some((target_line, 0));
                    self.state = VmState::Running;
                } else {
                    let err = format!("GOSUB 目标行号不存在: {}", target_line);
                    self.state = VmState::Error(err);
                }
            }
            Statement::Return => {
                if let Some(ret_pc) = self.call_stack.pop() {
                    self.pc = ret_pc;
                    self.state = VmState::Running;
                } else {
                    self.pc = next_pc;
                    self.state = VmState::Running;
                }
            }
            Statement::IfThen { lhs, op, rhs, stmt } => {
                let l = lhs.eval(&self.variables);
                let r = rhs.eval(&self.variables);
                let cond_met = match op {
                    CondOp::Eq => l == r,
                    CondOp::Ne => l != r,
                    CondOp::Lt => l < r,
                    CondOp::Gt => l > r,
                    CondOp::Le => l <= r,
                    CondOp::Ge => l >= r,
                };

                if cond_met {
                    // Check if this is a delay wait loop:
                    // e.g. `IF Y < X THEN GOTO 1802` where target line has `CALL TIME(...)`
                    if op == CondOp::Lt {
                        if let Statement::Goto(ref target_expr) = *stmt {
                            let target_line = target_expr.eval(&self.variables) as u32;
                            if target_line <= line_no {
                                if let Some(target_stmts) = self.program.lines.get(&target_line) {
                                    let is_time_loop = target_stmts
                                        .iter()
                                        .any(|s| matches!(s, Statement::CallTime(_)));
                                    if is_time_loop {
                                        if let Expr::Var(lhs_var) = lhs {
                                            self.set_variable(lhs_var, r);
                                        }
                                        self.pc = next_pc;
                                        self.state = VmState::WaitingForDelay { until_time: r };
                                        return self.state.clone();
                                    }
                                }
                            }
                        }
                    }

                    self.execute_sub_statement(*stmt, next_pc, host);
                } else {
                    self.pc = next_pc;
                    self.state = VmState::Running;
                }
            }
            Statement::ForTo { var, start, end } => {
                let s_val = start.eval(&self.variables);
                let e_val = end.eval(&self.variables);
                self.set_variable(var, s_val);
                self.for_stack.push(ForLoopState {
                    var,
                    end: e_val,
                    loop_start_pc: next_pc,
                });
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::Next(var) => {
                if let Some(loop_st) = self.for_stack.last().cloned() {
                    if loop_st.var == var {
                        let cur = self.get_variable(var) + 1;
                        self.set_variable(var, cur);
                        if cur <= loop_st.end {
                            self.pc = loop_st.loop_start_pc;
                            self.state = VmState::Running;
                            return self.state.clone();
                        } else {
                            self.for_stack.pop();
                        }
                    }
                }
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::End => {
                self.pc = None;
                self.state = VmState::Finished;
            }
            Statement::Rem(_) => {
                self.pc = next_pc;
                self.state = VmState::Running;
            }
            Statement::Unknown { raw, reason } => {
                self.pc = next_pc;
                let alert = UnrecognizedInstruction {
                    line_no,
                    raw_code: raw,
                    reason,
                };
                self.state = VmState::PausedForAlert(alert);
            }
        }

        self.state.clone()
    }

    fn execute_sub_statement(
        &mut self,
        stmt: Statement,
        fallback_next_pc: Option<(u32, usize)>,
        host: &mut dyn VmHost,
    ) {
        match stmt {
            Statement::Goto(expr) => {
                let target_line = expr.eval(&self.variables) as u32;
                if self.program.lines.contains_key(&target_line) {
                    self.pc = Some((target_line, 0));
                    self.state = VmState::Running;
                } else {
                    self.state = VmState::Error(format!("GOTO 目标行号不存在: {}", target_line));
                }
            }
            Statement::Gosub(expr) => {
                let target_line = expr.eval(&self.variables) as u32;
                if self.program.lines.contains_key(&target_line) {
                    self.call_stack.push(fallback_next_pc);
                    self.pc = Some((target_line, 0));
                    self.state = VmState::Running;
                } else {
                    self.state = VmState::Error(format!("GOSUB 目标行号不存在: {}", target_line));
                }
            }
            Statement::Return => {
                if let Some(ret_pc) = self.call_stack.pop() {
                    self.pc = ret_pc;
                } else {
                    self.pc = fallback_next_pc;
                }
                self.state = VmState::Running;
            }
            Statement::Assign(var, expr) => {
                let val = expr.eval(&self.variables);
                self.set_variable(var, val);
                self.pc = fallback_next_pc;
                self.state = VmState::Running;
            }
            Statement::DrawImage { file, x, y, mode } => {
                let x_val = x.eval(&self.variables);
                let y_val = y.eval(&self.variables);
                let mode_val = mode.eval(&self.variables);
                host.draw_image(&file, x_val, y_val, mode_val);
                self.pc = fallback_next_pc;
                self.state = VmState::Running;
            }
            Statement::PlaySound(file) => {
                host.play_sound(&file);
                self.pc = fallback_next_pc;
                self.state = VmState::Running;
            }
            Statement::End => {
                self.pc = None;
                self.state = VmState::Finished;
            }
            _ => {
                self.pc = fallback_next_pc;
                self.state = VmState::Running;
            }
        }
    }

    /// Runs until the VM yields (waiting for key, waiting for delay, paused for alert, error, or finished).
    pub fn run_until_yield(&mut self, host: &mut dyn VmHost) -> VmState {
        if let VmState::WaitingForDelay { until_time } = self.state {
            if host.get_time_units() < until_time {
                return self.state.clone();
            } else {
                self.state = VmState::Running;
            }
        }

        if matches!(self.state, VmState::WaitingForVideo) {
            return self.state.clone();
        }

        const MAX_STEPS_PER_TICK: usize = 100_000;
        let mut steps = 0;

        while self.pc.is_some() && steps < MAX_STEPS_PER_TICK {
            steps += 1;
            let res = self.step(host);
            match res {
                VmState::WaitingForKey { .. }
                | VmState::WaitingForDelay { .. }
                | VmState::WaitingForVideo
                | VmState::PausedForAlert(_)
                | VmState::Finished
                | VmState::Error(_) => return res,
                VmState::Running | VmState::Ready => {}
            }
        }

        if steps >= MAX_STEPS_PER_TICK {
            self.state = VmState::Error("执行步数超出单帧上限 (可能存在死循环)".to_string());
        }

        self.state.clone()
    }

    /// Injects key value when waiting for key input.
    pub fn inject_key(&mut self, key_code: i32, host: &mut dyn VmHost) -> VmState {
        if let VmState::WaitingForKey { target_var } = self.state {
            self.set_variable(target_var, key_code);
            self.state = VmState::Running;
            self.run_until_yield(host)
        } else {
            self.state.clone()
        }
    }

    /// Resumes execution after user dismisses an unrecognized instruction modal alert.
    pub fn skip_unrecognized_and_continue(&mut self, host: &mut dyn VmHost) -> VmState {
        if matches!(self.state, VmState::PausedForAlert(_)) {
            self.state = VmState::Running;
            self.run_until_yield(host)
        } else {
            self.state.clone()
        }
    }

    /// Terminates current script execution.
    pub fn terminate(&mut self) {
        self.pc = None;
        self.call_stack.clear();
        self.for_stack.clear();
        self.state = VmState::Finished;
    }
}

impl Default for VcdScriptVm {
    fn default() -> Self {
        Self::new()
    }
}
