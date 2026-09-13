# vcd30player Locales & Community Translations

This folder contains UI localization files for `vcd30player`.

## Built-in Languages
- `zh-CN.json`: 简体中文 (Simplified Chinese)
- `en-US.json`: English

## How to Add a Community Translation

1. Copy `_template.json` and rename it to your target IETF language tag (e.g., `ja-JP.json`, `es-ES.json`, `fr-FR.json`, `de-DE.json`, `ru-RU.json`).
2. Update the `_meta` section:
   ```json
   "_meta": {
     "locale": "ja-JP",
     "name": "日本語",
     "author": "YourName"
   }
   ```
3. Translate all values in the file into your language. Placeholders like `{0}` or `{1}` represent dynamic arguments and should be kept in position.
4. **Testing without compiling**:
   Place your translated `.json` file into the `locales/` folder next to the `vcd30player` executable, or in your user configuration directory:
   - Windows: `%APPDATA%\vcd30player\locales\`
   - Linux: `~/.config/vcd30player/locales/`
   Launch `vcd30player`, click the Settings icon (`⚙`) in the title bar, and your language will appear in the language selection list.
5. **Submitting your translation**:
   Submit a Pull Request to include your `.json` file in the official repository!
