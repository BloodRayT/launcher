"""
Update Manager for FenixCraft Launcher
Handles launcher updates and mod synchronization
"""

import json
import hashlib
import shutil
from pathlib import Path
from typing import Dict, List, Optional, Any
import requests


class UpdateManager:
    """Manages launcher and mod updates"""
    
    def __init__(self, config_manager):
        """Initialize update manager
        
        Args:
            config_manager: ConfigManager instance
        """
        self.config = config_manager
        self.update_server_url = config_manager.get("update_server_url", "")
        self.mods_server_url = config_manager.get("mods_server_url", "")
        self.launcher_dir = Path(__file__).parent.parent
        self.minecraft_dir = Path.home() / ".fenixcraft" / "minecraft"
        self.mods_dir = self.minecraft_dir / "mods"
        
        # Create directories
        self.mods_dir.mkdir(parents=True, exist_ok=True)
    
    def check_launcher_update(self) -> Optional[Dict[str, Any]]:
        """Check for launcher updates
        
        Returns:
            Update info dict if update available, None otherwise
        """
        if not self.update_server_url:
            return None
        
        try:
            response = requests.get(
                self.update_server_url,
                params={"version": self.config.launcher_version},
                timeout=30
            )
            response.raise_for_status()
            
            data = response.json()
            
            if data.get("has_update", False):
                return data
            
            return None
            
        except Exception as e:
            print(f"Error checking launcher update: {e}")
            return None
    
    def download_launcher_update(self, update_info: Dict[str, Any], progress_callback=None) -> bool:
        """Download launcher update
        
        Args:
            update_info: Update information from server
            progress_callback: Optional progress callback
            
        Returns:
            True if download successful
        """
        try:
            download_url = update_info.get("download_url")
            if not download_url:
                return False
            
            response = requests.get(download_url, stream=True, timeout=300)
            response.raise_for_status()
            
            total_size = int(response.headers.get('content-length', 0))
            
            # Save to temporary location
            update_file = self.launcher_dir / "launcher_update.jar"
            
            with open(update_file, 'wb') as f:
                downloaded = 0
                for chunk in response.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback:
                            progress_callback(downloaded, total_size)
            
            # Verify checksum if provided
            expected_hash = update_info.get("sha256")
            if expected_hash:
                actual_hash = self._calculate_sha256(update_file)
                if actual_hash != expected_hash:
                    update_file.unlink()
                    return False
            
            return True
            
        except Exception as e:
            print(f"Error downloading launcher update: {e}")
            return False
    
    def _calculate_sha256(self, file_path: Path) -> str:
        """Calculate SHA256 hash of file
        
        Args:
            file_path: Path to file
            
        Returns:
            SHA256 hash as hex string
        """
        sha256_hash = hashlib.sha256()
        with open(file_path, "rb") as f:
            for byte_block in iter(lambda: f.read(4096), b""):
                sha256_hash.update(byte_block)
        return sha256_hash.hexdigest()
    
    def get_available_mods(self) -> List[Dict[str, Any]]:
        """Get list of available mods from server
        
        Returns:
            List of mod information dicts
        """
        if not self.mods_server_url:
            return []
        
        try:
            response = requests.get(
                self.mods_server_url,
                params={
                    "version": self.config.game_version,
                    "mod_loader": self.config.mod_loader
                },
                timeout=30
            )
            response.raise_for_status()
            
            return response.json().get("mods", [])
            
        except Exception as e:
            print(f"Error fetching mods list: {e}")
            return []
    
    def sync_mods(self, progress_callback=None) -> Dict[str, List[str]]:
        """Synchronize local mods with server
        
        Args:
            progress_callback: Optional progress callback
            
        Returns:
            Dict with 'downloaded', 'removed', 'updated' lists
        """
        result = {
            "downloaded": [],
            "removed": [],
            "updated": []
        }
        
        try:
            # Get server mods list
            server_mods = self.get_available_mods()
            server_mod_files = {mod["filename"]: mod for mod in server_mods}
            
            # Get local mods
            local_mods = {}
            if self.mods_dir.exists():
                for mod_file in self.mods_dir.glob("*.jar"):
                    local_mods[mod_file.name] = {
                        "path": mod_file,
                        "hash": self._calculate_sha256(mod_file)
                    }
            
            # Download new or updated mods
            for filename, mod_info in server_mod_files.items():
                mod_path = self.mods_dir / filename
                
                needs_download = False
                
                if filename not in local_mods:
                    needs_download = True
                    result["downloaded"].append(filename)
                elif local_mods[filename]["hash"] != mod_info.get("sha256"):
                    needs_download = True
                    result["updated"].append(filename)
                
                if needs_download:
                    self._download_mod(mod_info, mod_path, progress_callback)
            
            # Remove mods not on server (optional - commented out for safety)
            # for filename, mod_info in local_mods.items():
            #     if filename not in server_mod_files:
            #         mod_info["path"].unlink()
            #         result["removed"].append(filename)
            
            return result
            
        except Exception as e:
            print(f"Error syncing mods: {e}")
            return result
    
    def _download_mod(self, mod_info: Dict[str, Any], mod_path: Path, progress_callback=None) -> bool:
        """Download a mod file
        
        Args:
            mod_info: Mod information from server
            mod_path: Path to save mod
            progress_callback: Optional progress callback
            
        Returns:
            True if download successful
        """
        try:
            download_url = mod_info.get("download_url")
            if not download_url:
                return False
            
            response = requests.get(download_url, stream=True, timeout=120)
            response.raise_for_status()
            
            total_size = int(response.headers.get('content-length', 0))
            
            with open(mod_path, 'wb') as f:
                downloaded = 0
                for chunk in response.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback:
                            progress_callback(downloaded, total_size)
            
            # Verify checksum
            expected_hash = mod_info.get("sha256")
            if expected_hash:
                actual_hash = self._calculate_sha256(mod_path)
                if actual_hash != expected_hash:
                    mod_path.unlink()
                    return False
            
            return True
            
        except Exception as e:
            print(f"Error downloading mod {mod_info.get('filename', 'unknown')}: {e}")
            if mod_path.exists():
                mod_path.unlink()
            return False
    
    def get_minecraft_resources(self) -> Dict[str, Any]:
        """Get Minecraft version manifest and NeoForge installer info
        
        Returns:
            Dict with version manifest and forge installer URL
        """
        # Minecraft version manifest
        minecraft_manifest_url = "https://piston-meta.mojang.com/mc/game/version_manifest_v2.json"
        
        try:
            # Get Minecraft version info
            manifest_response = requests.get(minecraft_manifest_url, timeout=30)
            manifest_response.raise_for_status()
            manifest = manifest_response.json()
            
            version_info = None
            for version in manifest.get("versions", []):
                if version["id"] == self.config.game_version:
                    version_info = version
                    break
            
            if not version_info:
                raise ValueError(f"Minecraft version {self.config.game_version} not found")
            
            # Get version details
            version_details_response = requests.get(version_info["url"], timeout=30)
            version_details_response.raise_for_status()
            version_details = version_details_response.json()
            
            # NeoForge installer URL (using official NeoForge maven)
            neoforge_version = self._get_neoforge_version()
            forge_installer_url = (
                f"https://maven.neoforged.net/releases/net/neoforged/forge/"
                f"{self.config.game_version}-{neoforge_version}/"
                f"{self.config.game_version}-{neoforge_version}-installer.jar"
            )
            
            return {
                "version_details": version_details,
                "forge_installer_url": forge_installer_url,
                "neoforge_version": neoforge_version
            }
            
        except Exception as e:
            print(f"Error getting Minecraft resources: {e}")
            return {}
    
    def _get_neoforge_version(self) -> str:
        """Get recommended NeoForge version for current Minecraft version
        
        Returns:
            NeoForge version string
        """
        # For 1.21.1, use the latest recommended version
        # In production, this should query the NeoForge API
        return "21.1.79"  # Example version, should be updated dynamically
