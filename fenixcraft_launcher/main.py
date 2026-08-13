"""
FenixCraft Launcher - Main Entry Point
Cross-platform Minecraft NeoForge 1.21.1 Launcher
"""

import sys
import argparse
from pathlib import Path

from fenixcraft_launcher.config import ConfigManager
from fenixcraft_launcher.java_manager import JavaManager
from fenixcraft_launcher.update_manager import UpdateManager
from fenixcraft_launcher.library_manager import LibraryManager
from fenixcraft_launcher.minecraft_launcher import MinecraftLauncher
from fenixcraft_launcher.main_window import MainWindow

from PyQt6.QtWidgets import QApplication
from PyQt6.QtCore import Qt


def main():
    """Main entry point for FenixCraft Launcher"""
    parser = argparse.ArgumentParser(description="FenixCraft Launcher")
    parser.add_argument("--config", type=str, help="Path to configuration file")
    parser.add_argument("--headless", action="store_true", help="Run in headless mode")
    args = parser.parse_args()
    
    # Initialize configuration
    config_manager = ConfigManager(args.config)
    
    print(f"Starting {config_manager.launcher_name} v{config_manager.launcher_version}")
    print(f"Target: Minecraft {config_manager.game_version} with NeoForge")
    print(f"Server: {config_manager.server_address}:{config_manager.server_port}")
    
    # Initialize managers
    java_manager = JavaManager()
    update_manager = UpdateManager(config_manager)
    library_manager = LibraryManager(config_manager)
    minecraft_launcher = MinecraftLauncher(config_manager, java_manager)
    
    if args.headless:
        # Headless mode - just launch
        run_headless(java_manager, update_manager, library_manager, minecraft_launcher)
    else:
        # GUI mode
        run_gui(config_manager, java_manager, update_manager, library_manager, minecraft_launcher)


def run_headless(java_manager, update_manager, library_manager, minecraft_launcher):
    """Run launcher in headless mode"""
    print("Running in headless mode...")
    
    # Check/install Java
    java_path = java_manager.get_java_path()
    if not java_path:
        print("ERROR: Could not find or install Java 21")
        sys.exit(1)
    
    print(f"Java found: {java_path}")
    
    # Sync mods
    print("Synchronizing mods...")
    sync_result = update_manager.sync_mods()
    print(f"Downloaded: {len(sync_result['downloaded'])} mods")
    print(f"Updated: {len(sync_result['updated'])} mods")
    
    # Launch game
    print("Launching Minecraft...")
    success = minecraft_launcher.launch()
    
    if success:
        print("Game launched successfully!")
    else:
        print("Failed to launch game")
        sys.exit(1)


def run_gui(config_manager, java_manager, update_manager, library_manager, minecraft_launcher):
    """Run launcher with GUI"""
    app = QApplication(sys.argv)
    app.setApplicationName(config_manager.launcher_name)
    
    # Enable High DPI scaling
    app.setHighDpiScaleFactorRoundingPolicy(
        Qt.HighDpiScaleFactorRoundingPolicy.PassThrough
    )
    
    # Create and show main window
    window = MainWindow(
        config_manager,
        java_manager,
        update_manager,
        library_manager,
        minecraft_launcher
    )
    window.show()
    
    # Perform initial checks
    check_java_status(window, java_manager)
    
    sys.exit(app.exec())


def check_java_status(window, java_manager):
    """Check Java status and update UI"""
    java_path = java_manager.find_installed_java()
    
    if java_path:
        window.java_status_label.setText(f"✓ Java 21 found: {java_path}")
        window.java_status_label.setStyleSheet("color: #00FF00; font-size: 14px;")
    else:
        window.java_status_label.setText("⚠ Java 21 not found - will download on first launch")
        window.java_status_label.setStyleSheet("color: #FFA500; font-size: 14px;")


if __name__ == "__main__":
    main()
