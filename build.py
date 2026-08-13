"""
Build script for FenixCraft Launcher
Creates executable JAR file for cross-platform distribution
"""

import subprocess
import sys
import os
from pathlib import Path


def build_jar():
    """Build launcher as JAR file using PyInstaller and jar packaging"""
    
    print("=" * 60)
    print("FenixCraft Launcher - Build Script")
    print("=" * 60)
    
    # Get project root
    project_root = Path(__file__).parent
    dist_dir = project_root / "dist"
    build_dir = project_root / "build"
    
    # Clean previous builds
    print("\n[1/5] Cleaning previous builds...")
    if dist_dir.exists():
        import shutil
        shutil.rmtree(dist_dir)
    if build_dir.exists():
        shutil.rmtree(build_dir)
    
    dist_dir.mkdir(parents=True, exist_ok=True)
    
    # Build with PyInstaller
    print("\n[2/5] Building with PyInstaller...")
    pyinstaller_args = [
        sys.executable, "-m", "PyInstaller",
        "--name", "FenixCraftLauncher",
        "--onefile",
        "--windowed",
        "--clean",
        "--noconfirm",
        f"--distpath={dist_dir}",
        f"--workpath={build_dir}",
        str(project_root / "fenixcraft_launcher" / "main.py")
    ]
    
    result = subprocess.run(pyinstaller_args, capture_output=False)
    if result.returncode != 0:
        print("ERROR: PyInstaller build failed!")
        sys.exit(1)
    
    # Create launcher JAR wrapper
    print("\n[3/5] Creating JAR launcher...")
    
    # Create manifest
    manifest_content = """Manifest-Version: 1.0
Main-Class: FenixCraftLauncher
Specification-Title: FenixCraft Launcher
Specification-Version: 1.0.0
Implementation-Title: FenixCraft Minecraft Launcher
Implementation-Version: 1.0.0
Created-By: FenixCraft Team

"""
    
    jar_dir = dist_dir / "jar_build"
    jar_dir.mkdir(parents=True, exist_ok=True)
    
    manifest_path = jar_dir / "MANIFEST.MF"
    with open(manifest_path, 'w') as f:
        f.write(manifest_content)
    
    # Copy executable to jar build directory
    if sys.platform == "win32":
        exe_name = "FenixCraftLauncher.exe"
    else:
        exe_name = "FenixCraftLauncher"
    
    exe_path = dist_dir / exe_name
    if not exe_path.exists():
        print(f"ERROR: Executable not found at {exe_path}")
        sys.exit(1)
    
    import shutil
    shutil.copy(exe_path, jar_dir / exe_name)
    
    # Copy required resources
    resources_dir = jar_dir / "resources"
    resources_dir.mkdir(parents=True, exist_ok=True)
    
    config_src = project_root / "config.json"
    if config_src.exists():
        shutil.copy(config_src, resources_dir / "config.json")
    
    # Create launch script for JAR
    print("\n[4/5] Creating launch scripts...")
    
    # Windows batch file
    bat_content = f"""@echo off
title FenixCraft Launcher
echo Starting FenixCraft Launcher...
cd /d "%~dp0"
start "" "{exe_name}"
exit
"""
    
    bat_path = dist_dir / "FenixCraftLauncher.bat"
    with open(bat_path, 'w') as f:
        f.write(bat_content)
    
    # Unix shell script
    sh_content = f"""#!/bin/bash
# FenixCraft Launcher
echo "Starting FenixCraft Launcher..."
cd "$(dirname "$0")"
chmod +x {exe_name}
./{exe_name} &
"""
    
    sh_path = dist_dir / "FenixCraftLauncher.sh"
    with open(sh_path, 'w') as f:
        f.write(sh_content)
    os.chmod(sh_path, 0o755)
    
    # macOS app bundle (optional)
    if sys.platform == "darwin":
        create_macos_app_bundle(dist_dir, exe_name)
    
    # Create README for distribution
    print("\n[5/5] Creating distribution package...")
    
    readme_content = f"""# FenixCraft Launcher v1.0.0

Minecraft NeoForge 1.21.1 Launcher for fenixcraft.ru server

## Features
- Automatic Java 21 installation
- Auto-update launcher and mods
- Direct connect to fenixcraft.ru:25565
- Cross-platform support (Windows, Linux, macOS)

## Installation

### Windows
1. Run `FenixCraftLauncher.exe` or double-click `FenixCraftLauncher.bat`

### Linux
1. Make executable: `chmod +x FenixCraftLauncher`
2. Run: `./FenixCraftLauncher` or use `FenixCraftLauncher.sh`

### macOS
1. Open `FenixCraftLauncher.app`
2. Or run from terminal: `./FenixCraftLauncher`

## Requirements
- No external dependencies required!
- Java 21 will be automatically downloaded if not installed

## Server
- Address: fenixcraft.ru
- Port: 25565
- Version: Minecraft 1.21.1 with NeoForge

## Support
Visit fenixcraft.ru for support and updates.

---
Created by FenixCraft Team
"""
    
    readme_path = dist_dir / "README.txt"
    with open(readme_path, 'w') as f:
        f.write(readme_content)
    
    # Summary
    print("\n" + "=" * 60)
    print("BUILD COMPLETE!")
    print("=" * 60)
    print(f"\nDistribution directory: {dist_dir}")
    print("\nFiles created:")
    for item in dist_dir.iterdir():
        if item.is_file():
            size = item.stat().st_size / (1024 * 1024)  # MB
            print(f"  - {item.name} ({size:.2f} MB)")
    
    print("\nTo create a JAR archive:")
    print(f"  cd {jar_dir}")
    print("  jar cfm ../FenixCraftLauncher.jar MANIFEST.MF *")
    
    return True


def create_macos_app_bundle(dist_dir: Path, exe_name: str):
    """Create macOS application bundle"""
    
    app_dir = dist_dir / "FenixCraftLauncher.app" / "Contents"
    app_dir.mkdir(parents=True, exist_ok=True)
    
    # Info.plist
    info_plist = f"""<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{exe_name}</string>
    <key>CFBundleIconFile</key>
    <string></string>
    <key>CFBundleIdentifier</key>
    <string>ru.fenixcraft.launcher</string>
    <key>CFBundleName</key>
    <string>FenixCraft Launcher</string>
    <key>CFBundleVersion</key>
    <string>1.0.0</string>
    <key>CFBundleShortVersionString</key>
    <string>1.0.0</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"""
    
    with open(app_dir / "Info.plist", 'w') as f:
        f.write(info_plist)
    
    # Copy executable
    import shutil
    exe_path = dist_dir / exe_name
    if exe_path.exists():
        shutil.copy(exe_path, app_dir / "MacOS" / exe_name)
        (app_dir / "MacOS").mkdir(parents=True, exist_ok=True)
        shutil.move(app_dir / "MacOS" / exe_name, app_dir / "MacOS" / exe_name)


if __name__ == "__main__":
    success = build_jar()
    sys.exit(0 if success else 1)
