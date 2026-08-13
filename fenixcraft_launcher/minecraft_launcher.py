"""
Minecraft Launcher for FenixCraft
Handles launching Minecraft with NeoForge
"""

import subprocess
import sys
import json
from pathlib import Path
from typing import List, Optional, Dict, Any


class MinecraftLauncher:
    """Launches Minecraft with NeoForge"""
    
    def __init__(self, config_manager, java_manager):
        """Initialize Minecraft launcher
        
        Args:
            config_manager: ConfigManager instance
            java_manager: JavaManager instance
        """
        self.config = config_manager
        self.java_manager = java_manager
        self.minecraft_dir = Path.home() / ".fenixcraft" / "minecraft"
        self.game_dir = self.minecraft_dir / "game"
        
        # Create game directory
        self.game_dir.mkdir(parents=True, exist_ok=True)
    
    def launch(self, progress_callback=None) -> bool:
        """Launch Minecraft
        
        Args:
            progress_callback: Optional callback for launch status
            
        Returns:
            True if launch successful
        """
        try:
            # Get Java executable
            java_executable = self.java_manager.get_java_executable()
            if not java_executable:
                print("Java 21 not found. Please install Java 21.")
                return False
            
            # Build command line
            args = self._build_command()
            
            if progress_callback:
                progress_callback("Launching Minecraft...")
            
            # Launch game
            process = subprocess.Popen(
                args,
                cwd=str(self.game_dir),
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                text=True
            )
            
            # Log output
            for line in process.stderr:
                print(line.strip())
            
            return True
            
        except Exception as e:
            print(f"Error launching Minecraft: {e}")
            return False
    
    def _build_command(self) -> List[str]:
        """Build Minecraft launch command
        
        Returns:
            List of command arguments
        """
        version_id = f"{self.config.game_version}-neoforge"
        version_json = self.minecraft_dir / "versions" / version_id / f"{version_id}.json"
        
        # Load version JSON
        with open(version_json, 'r') as f:
            version_data = json.load(f)
        
        # Get main class
        main_class = version_data.get("mainClass", "net.minecraft.client.main.Main")
        
        # Build classpath
        classpath = self._build_classpath(version_data)
        
        # Get JVM arguments
        jvm_args = self._get_jvm_arguments(version_data)
        
        # Get game arguments
        game_args = self._get_game_arguments(version_data)
        
        # Build final command
        args = [
            str(self.java_manager.java_path),
            *jvm_args,
            "-cp",
            classpath,
            main_class,
            *game_args
        ]
        
        return args
    
    def _build_classpath(self, version_data: Dict[str, Any]) -> str:
        """Build classpath from libraries
        
        Args:
            version_data: Version JSON data
            
        Returns:
            Classpath string
        """
        libraries = []
        
        # Add Minecraft jar
        minecraft_jar = self.minecraft_dir / "versions" / version_data["id"] / f"{version_data['id']}.jar"
        if minecraft_jar.exists():
            libraries.append(str(minecraft_jar))
        
        # Add libraries
        libraries_dir = self.minecraft_dir / "libraries"
        for lib in version_data.get("libraries", []):
            lib_name = lib.get("name", "")
            if not lib_name:
                continue
            
            parts = lib_name.split(":")
            if len(parts) < 3:
                continue
            
            group, name, version = parts[0], parts[1], parts[2]
            lib_path = libraries_dir / group.replace(".", "/") / name / version / f"{name}-{version}.jar"
            
            if lib_path.exists():
                libraries.append(str(lib_path))
        
        # Join with platform-specific separator
        if sys.platform == "win32":
            return ";".join(libraries)
        else:
            return ":".join(libraries)
    
    def _get_jvm_arguments(self, version_data: Dict[str, Any]) -> List[str]:
        """Get JVM arguments
        
        Args:
            version_data: Version JSON data
            
        Returns:
            List of JVM arguments
        """
        args = []
        
        # Memory settings
        min_ram = f"-Xmx{self.config.min_ram_mb}M"
        max_ram = f"-Xms{self.config.max_ram_mb}M"
        args.extend([min_ram, max_ram])
        
        # Standard JVM args
        args.extend([
            "-XX:+UseG1GC",
            "-XX:+ParallelRefProcEnabled",
            "-XX:MaxGCPauseMillis=200",
            "-Djava.library.path=" + str(self.minecraft_dir / "natives"),
            "-Dlog4j.configurationFile=log4j2.xml"
        ])
        
        # Add JVM args from version data
        for jvm_arg in version_data.get("arguments", {}).get("jvm", []):
            if isinstance(jvm_arg, str):
                args.append(jvm_arg)
        
        return args
    
    def _get_game_arguments(self, version_data: Dict[str, Any]) -> List[str]:
        """Get game arguments
        
        Args:
            version_data: Version JSON data
            
        Returns:
            List of game arguments
        """
        args = []
        
        # Standard game args
        args.extend([
            "--username", "FenixPlayer",
            "--version", version_data.get("id", self.config.game_version),
            "--gameDir", str(self.game_dir),
            "--assetsDir", str(self.minecraft_dir / "assets"),
            "--assetIndex", version_data.get("assetIndex", {}).get("id", "1.21"),
            "--uuid", "00000000-0000-0000-0000-000000000000",
            "--accessToken", "0",
            "--userType", "mojang",
            "--versionType", "release"
        ])
        
        # Server auto-connect
        if self.config.get("auto_connect", True):
            args.extend([
                "--server", self.config.server_address,
                "--port", str(self.config.server_port)
            ])
        
        # Add game args from version data
        for game_arg in version_data.get("arguments", {}).get("game", []):
            if isinstance(game_arg, str):
                args.append(game_arg)
        
        return args
