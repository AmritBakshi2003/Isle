# Changelog

All notable changes to Isle will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.4] - 2025-01-15

### Added
- Unicode-safe string handling for app names (fixes crash on multibyte characters)
- PowerShell-based app enumeration using Get-StartApps
- 5-minute in-memory cache for app enumeration
- Self-healing icon cache with automatic re-extraction
- Fallback to .lnk scanning when PowerShell fails
- Auto-focus search input when app picker opens
- Search input autofocus in app picker
- Character-safe string operations throughout codebase

### Changed
- Replaced byte-based string slicing with character-safe operations
- Improved search scoring algorithm (removed fuzzy subsequence matching)
- Updated app picker to use shell:AppsFolder\... paths for Store apps
- Simplified app enumeration to single source of truth (PowerShell)
- Removed folder-based app filtering
- Changed deduplication to use AppID instead of name

### Fixed
- Fixed crash on Unicode app names (e.g., "Microsoft® Word")
- Fixed incorrect Store app names showing as "Windows app"
- Fixed search returning false positives (e.g., "sound" matching "MySQL Server")
- Fixed modal overlay backdrop to be transparent
- Fixed app picker name rendering (was not showing)
- Fixed icon fallback to use app name's first character

### Security
- Added panic::catch_unwind wrapper around app enumeration
- Added validation for absolute file paths in clip_open_file
- All SQL queries use parameterized statements

### Performance
- Reduced polling by adding in-memory cache for app enumeration
- Icon extraction uses hash-based cache keys for reuse

### Code Quality
- Removed unused logger::error function
- Removed unused SystemState.auto_hide_idle field
- Added 15 unit tests for app enumeration and Unicode handling

## [0.2.3] - Previous Release

### Added
- Unified Tasks & Calendar view (NotchNook-style interface)
- Event reminder notifications
- Task reminder notifications
- Wallpaper rotation
- System state detection (fullscreen, battery saver, idle)
- Auto-hide on idle/inactivity
- Auto-hide on fullscreen apps
- Auto-hide on battery saver

### Changed
- Improved window positioning and DPI handling
- Updated CSP to block all remote content
- Improved clipboard history with file support

## [0.2.2] - Previous Release

### Added
- Clipboard history with image support
- Media controls (play/pause, next, previous)
- Progress ring for file operations
- Battery level indicator
- Customizable appearance settings
- Global hotkey support

## [0.2.1] - Previous Release

### Added
- Initial Dynamic Island-style UI
- Task management
- Calendar events
- Modes and routines
- Utility drawer with app shortcuts
- System tray integration

## [0.1.0] - Initial Release

### Added
- Basic Dynamic Island UI
- Task list
- Settings panel
- Tauri 2 + Svelte 5 architecture
