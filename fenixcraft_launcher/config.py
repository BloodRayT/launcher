"""
Configuration manager for FenixCraft Launcher
Handles loading and saving launcher settings
"""

import json
import os
from pathlib import Path
from typing import Dict, Any, Optional


class ConfigManager:
    """Manages launcher configuration"""
    
    DEFAULT_CONFIG = {
        "launcher_name": "FenixCraft Launcher",
        "launcher_version": "1.0.0",
        "game_version": "1.21.1",
        "mod_loader": "neoforge",
        "server_address": "fenixcraft.ru",
        "server_port": 25565,
        "java_version": "21",
        "java_download_url": "https://api.adoptium.net/v3/binary/latest/21/ga/{os}/{arch}/jdk/hotspot/normal/eclipse",
        "update_server_url": "https://fenixcraft.ru/api/launcher/update",
        "mods_server_url": "https://fenixcraft.ru/api/launcher/mods",
        "min_ram_mb": 2048,
        "max_ram_mb": 4096,
        "theme": {
            "primary_color": "#FF6B35",
            "secondary_color": "#1A1A2E",
            "accent_color": "#E94560",
            "text_color": "#FFFFFF",
            "background_gradient_start": "#0F0F1E",
            "background_gradient_end": "#1A1A2E"
        }
    }
    
    def __init__(self, config_path: Optional[str] = None):
        """Initialize configuration manager
        
        Args:
            config_path: Path to configuration file. If None, uses default location
        """
        if config_path is None:
            self.config_path = Path.home() / ".fenixcraft" / "config.json"
        else:
            self.config_path = Path(config_path)
        
        self.config: Dict[str, Any] = {}
        self._load_config()
    
    def _load_config(self) -> None:
        """Load configuration from file or create default"""
        if self.config_path.exists():
            try:
                with open(self.config_path, 'r', encoding='utf-8') as f:
                    self.config = json.load(f)
                # Merge with defaults to ensure all keys exist
                self._merge_with_defaults()
            except (json.JSONDecodeError, IOError) as e:
                print(f"Warning: Could not load config file: {e}")
                self.config = self.DEFAULT_CONFIG.copy()
                self.save_config()
        else:
            self.config = self.DEFAULT_CONFIG.copy()
            self.save_config()
    
    def _merge_with_defaults(self) -> None:
        """Merge current config with defaults to ensure completeness"""
        for key, value in self.DEFAULT_CONFIG.items():
            if key not in self.config:
                self.config[key] = value
            elif isinstance(value, dict) and isinstance(self.config[key], dict):
                for sub_key, sub_value in value.items():
                    if sub_key not in self.config[key]:
                        self.config[key][sub_key] = sub_value
    
    def save_config(self) -> None:
        """Save configuration to file"""
        self.config_path.parent.mkdir(parents=True, exist_ok=True)
        with open(self.config_path, 'w', encoding='utf-8') as f:
            json.dump(self.config, f, indent=4, ensure_ascii=False)
    
    def get(self, key: str, default: Any = None) -> Any:
        """Get configuration value
        
        Args:
            key: Configuration key (supports dot notation for nested keys)
            default: Default value if key doesn't exist
            
        Returns:
            Configuration value or default
        """
        keys = key.split('.')
        value = self.config
        for k in keys:
            if isinstance(value, dict) and k in value:
                value = value[k]
            else:
                return default
        return value
    
    def set(self, key: str, value: Any) -> None:
        """Set configuration value
        
        Args:
            key: Configuration key (supports dot notation for nested keys)
            value: Value to set
        """
        keys = key.split('.')
        config = self.config
        for k in keys[:-1]:
            if k not in config:
                config[k] = {}
            config = config[k]
        config[keys[-1]] = value
        self.save_config()
    
    @property
    def launcher_name(self) -> str:
        return self.config.get("launcher_name", "FenixCraft Launcher")
    
    @property
    def launcher_version(self) -> str:
        return self.config.get("launcher_version", "1.0.0")
    
    @property
    def game_version(self) -> str:
        return self.config.get("game_version", "1.21.1")
    
    @property
    def mod_loader(self) -> str:
        return self.config.get("mod_loader", "neoforge")
    
    @property
    def server_address(self) -> str:
        return self.config.get("server_address", "fenixcraft.ru")
    
    @property
    def server_port(self) -> int:
        return self.config.get("server_port", 25565)
    
    @property
    def java_version(self) -> str:
        return self.config.get("java_version", "21")
    
    @property
    def min_ram_mb(self) -> int:
        return self.config.get("min_ram_mb", 2048)
    
    @property
    def max_ram_mb(self) -> int:
        return self.config.get("max_ram_mb", 4096)
    
    @property
    def theme(self) -> Dict[str, str]:
        return self.config.get("theme", self.DEFAULT_CONFIG["theme"])
