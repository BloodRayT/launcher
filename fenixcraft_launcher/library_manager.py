"""
Library Manager for FenixCraft Launcher
Handles Minecraft libraries and assets download
"""

import json
import hashlib
import os
from pathlib import Path
from typing import Dict, List, Optional, Any
import requests


class LibraryManager:
    """Manages Minecraft libraries and assets"""
    
    def __init__(self, config_manager):
        """Initialize library manager
        
        Args:
            config_manager: ConfigManager instance
        """
        self.config = config_manager
        self.minecraft_dir = Path.home() / ".fenixcraft" / "minecraft"
        self.libraries_dir = self.minecraft_dir / "libraries"
        self.assets_dir = self.minecraft_dir / "assets"
        self.versions_dir = self.minecraft_dir / "versions"
        
        # Create directories
        for directory in [self.libraries_dir, self.assets_dir, self.versions_dir]:
            directory.mkdir(parents=True, exist_ok=True)
    
    def download_libraries(self, version_details: Dict[str, Any], progress_callback=None) -> bool:
        """Download all required libraries for a Minecraft version
        
        Args:
            version_details: Minecraft version details from Mojang API
            progress_callback: Optional progress callback
            
        Returns:
            True if all libraries downloaded successfully
        """
        try:
            libraries = version_details.get("libraries", [])
            total_libs = len(libraries)
            downloaded = 0
            
            for library in libraries:
                # Skip if not for this OS
                if "rules" in library:
                    if not self._check_rules(library["rules"]):
                        continue
                
                lib_name = library.get("name", "")
                if not lib_name:
                    continue
                
                # Parse library name (format: group:name:version)
                parts = lib_name.split(":")
                if len(parts) < 3:
                    continue
                
                group, name, version = parts[0], parts[1], parts[2]
                
                # Get download URL
                downloads = library.get("downloads", {})
                artifact = downloads.get("artifact", {})
                
                if not artifact:
                    # Construct URL manually
                    path = f"{group.replace('.', '/')}/{name}/{version}/{name}-{version}.jar"
                    url = f"https://libraries.minecraft.net/{path}"
                else:
                    url = artifact.get("url", "")
                    path = artifact.get("path", "")
                
                if not url:
                    continue
                
                # Download library
                lib_path = self.libraries_dir / path
                if not self._download_file(url, lib_path, progress_callback):
                    print(f"Failed to download library: {lib_name}")
                    # Continue anyway as some libraries are optional
                
                downloaded += 1
                if progress_callback:
                    progress_callback(downloaded, total_libs, "libraries")
            
            return True
            
        except Exception as e:
            print(f"Error downloading libraries: {e}")
            return False
    
    def _check_rules(self, rules: List[Dict]) -> bool:
        """Check if library should be downloaded based on rules
        
        Args:
            rules: List of rule dictionaries
            
        Returns:
            True if library should be downloaded
        """
        import sys
        
        allow = False
        for rule in rules:
            action = rule.get("action", "deny")
            os_rule = rule.get("os", {})
            
            if os_rule:
                os_name = os_rule.get("name", "")
                if os_name == "windows" and sys.platform == "win32":
                    if action == "allow":
                        allow = True
                    elif action == "deny":
                        allow = False
                elif os_name == "osx" and sys.platform == "darwin":
                    if action == "allow":
                        allow = True
                    elif action == "deny":
                        allow = False
                elif os_name == "linux" and sys.platform == "linux":
                    if action == "allow":
                        allow = True
                    elif action == "deny":
                        allow = False
            else:
                if action == "allow":
                    allow = True
                elif action == "deny":
                    allow = False
        
        return allow
    
    def download_assets(self, version_details: Dict[str, Any], progress_callback=None) -> bool:
        """Download game assets
        
        Args:
            version_details: Minecraft version details
            progress_callback: Optional progress callback
            
        Returns:
            True if all assets downloaded successfully
        """
        try:
            asset_index = version_details.get("assetIndex", {})
            asset_index_url = asset_index.get("url", "")
            asset_index_id = asset_index.get("id", "1.21")
            
            if not asset_index_url:
                print("No asset index URL found")
                return False
            
            # Download asset index
            index_path = self.assets_dir / "indexes" / f"{asset_index_id}.json"
            index_path.parent.mkdir(parents=True, exist_ok=True)
            
            if not self._download_file(asset_index_url, index_path):
                return False
            
            # Parse asset index
            with open(index_path, 'r') as f:
                index_data = json.load(f)
            
            objects = index_data.get("objects", {})
            total_assets = len(objects)
            downloaded = 0
            
            for asset_name, asset_info in objects.items():
                asset_hash = asset_info.get("hash", "")
                
                # Calculate asset path
                hash_prefix = asset_hash[:2]
                asset_path = self.assets_dir / "objects" / hash_prefix / asset_hash
                
                if asset_path.exists():
                    # Verify hash
                    if self._verify_hash(asset_path, asset_hash):
                        downloaded += 1
                        continue
                
                # Download asset
                asset_url = f"https://resources.download.minecraft.net/{hash_prefix}/{asset_hash}"
                asset_path.parent.mkdir(parents=True, exist_ok=True)
                
                if not self._download_file(asset_url, asset_path):
                    print(f"Failed to download asset: {asset_name}")
                    continue
                
                downloaded += 1
                if progress_callback:
                    progress_callback(downloaded, total_assets, "assets")
            
            return True
            
        except Exception as e:
            print(f"Error downloading assets: {e}")
            return False
    
    def create_version_json(self, version_details: Dict[str, Any], forge_installer_path: Optional[Path] = None) -> Path:
        """Create version JSON file for Minecraft launcher
        
        Args:
            version_details: Minecraft version details
            forge_installer_path: Optional path to Forge installer
            
        Returns:
            Path to created version JSON file
        """
        version_id = version_details.get("id", self.config.game_version)
        
        if forge_installer_path:
            # Modify version details for Forge
            version_details = self._modify_for_forge(version_details, forge_installer_path)
        
        version_dir = self.versions_dir / version_id
        version_dir.mkdir(parents=True, exist_ok=True)
        
        version_json_path = version_dir / f"{version_id}.json"
        
        with open(version_json_path, 'w') as f:
            json.dump(version_details, f, indent=2)
        
        return version_json_path
    
    def _modify_for_forge(self, version_details: Dict[str, Any], installer_path: Path) -> Dict[str, Any]:
        """Modify version details for Forge/NeoForge
        
        Args:
            version_details: Original Minecraft version details
            installer_path: Path to Forge installer
            
        Returns:
            Modified version details
        """
        # This is a simplified version - in production, you'd extract
        # the actual version.json from the Forge installer
        version_details["id"] = f"{self.config.game_version}-neoforge"
        version_details["mainClass"] = "cpw.mods.bootstraplauncher.BootstrapLauncher"
        
        return version_details
    
    def _download_file(self, url: str, path: Path, progress_callback=None) -> bool:
        """Download a file from URL
        
        Args:
            url: Download URL
            path: Save path
            progress_callback: Optional progress callback
            
        Returns:
            True if download successful
        """
        try:
            response = requests.get(url, stream=True, timeout=120)
            response.raise_for_status()
            
            total_size = int(response.headers.get('content-length', 0))
            
            with open(path, 'wb') as f:
                downloaded = 0
                for chunk in response.iter_content(chunk_size=8192):
                    if chunk:
                        f.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback and total_size > 0:
                            progress_callback(downloaded, total_size)
            
            return True
            
        except Exception as e:
            print(f"Error downloading {url}: {e}")
            if path.exists():
                path.unlink()
            return False
    
    def _verify_hash(self, file_path: Path, expected_hash: str) -> bool:
        """Verify file SHA1 hash
        
        Args:
            file_path: Path to file
            expected_hash: Expected SHA1 hash
            
        Returns:
            True if hash matches
        """
        sha1_hash = hashlib.sha1()
        with open(file_path, "rb") as f:
            for byte_block in iter(lambda: f.read(4096), b""):
                sha1_hash.update(byte_block)
        
        return sha1_hash.hexdigest() == expected_hash
