# QuickRDP

A fast and efficient RDP connection manager for Windows system administrators. QuickRDP provides secure credential storage, Active Directory integration, and quick server search capabilities.

![License](https://img.shields.io/badge/license-MIT-blue.svg)
![Platform](https://img.shields.io/badge/platform-Windows-blue.svg)
![Tauri](https://img.shields.io/badge/Tauri-2.0-brightgreen.svg)

## Features

### 🔐 Secure Credential Management
- Store credentials securely using Windows Credential Manager
- Support for per-host credentials or global credentials
- Credentials persist across sessions without storing in plain text

### 🚀 Fast Server Access
- Quick search and filter through your RDP hosts
- One-click connection to saved servers
- System tray integration for quick access
- Automatically saves connection files for faster subsequent connections

### 📋 Active Directory Integration
- Scan your Active Directory domain for Windows servers
- Automatically discover and import server information
- Filter and add servers directly from AD scan results

### 🎨 Modern UI
- Clean, responsive interface built with Tailwind CSS and DaisyUI
- Multiple theme options
- Intuitive host management with descriptions
- Modal-based host editing

### 🔧 Advanced Features
- **Per-host credentials**: Set different credentials for specific servers
- **Autostart support**: Launch QuickRDP on Windows startup
- **Debug logging**: Enable detailed logging with `--debug` flag
- **Application reset**: Secret keyboard shortcut (Ctrl+Shift+Alt+R) to completely reset the app
- **RDP file management**: Persistent connection files stored in AppData

## Installation

### Prerequisites
- Windows 10/11 (x64)
- Node.js (LTS version recommended)
- Rust toolchain

### Building from Source

1. **Clone the repository**
   ```powershell
   git clone <repository-url>
   cd QuickRDP-main
   ```

2. **Install dependencies**
   ```powershell
   npm install
   ```

3. **Run in development mode**
   ```powershell
   npm run tauri dev
   ```

4. **Build for production**
   ```powershell
   npm run tauri build
   ```

   The installer will be created in `src-tauri/target/release/bundle/`

## Usage

### First Launch
1. Launch QuickRDP
2. Enter your domain credentials (format: `DOMAIN\username` or `username@domain.com`)
3. Click OK to save credentials

### Adding Hosts Manually
1. Click "Manage Hosts" from the main window
2. Click "Add Host" button
3. Enter hostname (FQDN format: `server.domain.com`)
4. Add optional description
5. Click Save

### Scanning Active Directory
1. Click "Manage Hosts"
2. Click "Scan Domain"
3. Enter your domain name (e.g., `example.com`)
4. Enter your domain controller (e.g., `dc01.example.com`)
5. Click Scan to discover Windows servers
6. Select servers and click "Add Selected" to import

### Connecting to Hosts
1. Search for a host in the main window
2. Click on the host card to connect
3. RDP connection will launch automatically

### Per-Host Credentials
1. Right-click on a host (or use the host context menu)
2. Select "Set Credentials"
3. Enter specific credentials for that host
4. These override global credentials for that server

### Debug Mode
Enable detailed logging for troubleshooting:
```powershell
QuickRDP.exe --debug
```

Logs are written to:
- `%APPDATA%\Roaming\QuickRDP\QuickRDP.log`
- `%APPDATA%\Roaming\QuickRDP\QuickRDP_Debug_Log.txt`

### Application Reset
Press **Ctrl+Shift+Alt+R** to completely reset the application:
- Deletes all stored credentials
- Removes all RDP connection files
- Clears the hosts list
- Returns app to initial state

## Technical Details

### Tech Stack
- **Frontend**: Vite + TypeScript + Tailwind CSS + DaisyUI
- **Backend**: Rust with Tauri 2.0
- **Windows Integration**: Win32 APIs for credentials and RDP

### Project Structure
```
QuickRDP-main/
├── src/                    # Frontend TypeScript source
│   ├── main.ts            # Main window logic
│   ├── hosts.ts           # Host management
│   └── styles.css         # Global styles
├── src-tauri/             # Rust backend
│   ├── src/
│   │   ├── main.rs        # Entry point
│   │   └── lib.rs         # Core application logic
│   ├── Cargo.toml         # Rust dependencies
│   └── tauri.conf.json    # Tauri configuration
├── index.html             # Login window
├── main.html              # Main window
├── hosts.html             # Host management window
└── package.json           # Node.js dependencies
```

### Data Storage
- **Credentials**: Windows Credential Manager (`TERMSRV/*` and `QuickRDP`)
- **Hosts**: CSV file at `src-tauri/hosts.csv`
- **RDP Files**: `%APPDATA%\Roaming\QuickRDP\Connections\`
- **Logs**: `%APPDATA%\Roaming\QuickRDP\` (when debug enabled)

## Development

### Development Mode
```powershell
npm run tauri dev
```
- Hot reload enabled
- Debug mode on by default
- Console logging available

### Building Release
```powershell
npm run tauri build
```
- Optimized binary
- No logging unless `--debug` flag is used
- Creates installer in `src-tauri/target/release/bundle/`

### Code Structure
- **Tauri Commands**: Rust functions exposed to frontend via `#[tauri::command]`
- **Windows API Integration**: Direct Win32 calls for credential management
- **LDAP Support**: ldap3 crate for Active Directory queries

## Troubleshooting

### RDP Connection Fails
- Verify credentials are correct
- Check server hostname is reachable
- Ensure RDP is enabled on target server
- Try with `--debug` flag and check logs

### "Invalid Password" Errors
- Username format matters: try both `DOMAIN\user` and `user@domain.com`
- Verify credentials in Windows Credential Manager
- Check debug logs for credential parsing issues

### Active Directory Scan Fails
- Ensure domain controller is reachable
- Verify credentials have read access to AD
- Check that port 389 (LDAP) is not blocked
- Anonymous bind must be disabled on DC

### Application Won't Start
- Check Windows Event Viewer for errors
- Verify all dependencies are installed
- Try running with `--debug` flag
- Reset application with Ctrl+Shift+Alt+R

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Author

**Swatto**

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

## Acknowledgments

- Built with [Tauri](https://tauri.app/)
- UI components from [DaisyUI](https://daisyui.com/)
- Icons and styling with [Tailwind CSS](https://tailwindcss.com/)
