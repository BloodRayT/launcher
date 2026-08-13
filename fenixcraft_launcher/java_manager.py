"""
Java Manager for FenixCraft Launcher
Handles Java installation, detection, and management across platforms
"""

import os
import sys
import subprocess
import hashlib
import tempfile
from pathlib import Path
from typing import Optional, Tuple
import requests
from packaging.version import Version


class JavaManager:
    """Manages Java installation and detection"""
    
    JAVA_21_VERSION = "21"
    MIN_JAVA_VERSION = Version("21.0.0")
    
    def __init__(self, install_dir: Optional[Path] = None):
        """Initialize Java manager
        
        Args:
            install_dir: Directory to install Java. If None, uses launcher directory
        """
        if install_dir is None:
            self.install_dir = Path.home() / ".fenixcraft" / "java"
        else:
            self.install_dir = Path(install_dir)
        
        self.install_dir.mkdir(parents=True, exist_ok=True)
        self.java_path: Optional[Path] = None
    
    def get_platform_info(self) -> Tuple[str, str]:
        """Get platform information for Java download
        
        Returns:
            Tuple of (os_type, arch)
        """
        if sys.platform == "win32":
            os_type = "windows"
        elif sys.platform == "darwin":
            os_type = "mac"
        else:
            os_type = "linux"
        
        # Detect architecture
        import platform
        machine = platform.machine().lower()
        if machine in ("amd64", "x86_64"):
            arch = "x64"
        elif machine in ("arm64", "aarch64"):
            arch = "aarch64"
        else:
            arch = "x64"  # Default to x64
        
        return os_type, arch
    
    def get_java_download_url(self) -> str:
        """Get Java download URL for current platform
        
        Returns:
            Download URL for Java 21
        """
        os_type, arch = self.get_platform_info()
        return f"https://api.adoptium.net/v3/binary/latest/21/ga/{os_type}/{arch}/jdk/hotspot/normal/eclipse"
    
    def find_installed_java(self) -> Optional[Path]:
        """Find installed Java 21 on the system
        
        Returns:
            Path to Java executable if found, None otherwise
        """
        # Check local installation first
        local_java = self._check_local_java()
        if local_java:
            return local_java
        
        # Check system PATH
        java_executables = ["java", "java.exe"]
        for exe in java_executables:
            try:
                result = subprocess.run(
                    [exe, "-version"],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode == 0:
                    # Parse version from stderr (java -version outputs to stderr)
                    version_output = result.stderr or result.stdout
                    if "21" in version_output or "1.8" not in version_output:
                        # Try to get full version
                        try:
                            version_result = subprocess.run(
                                [exe, "-XshowSettings:properties", "-version"],
                                capture_output=True,
                                text=True,
                                timeout=10
                            )
                            for line in version_result.stderr.split('\n'):
                                if 'java.version' in line:
                                    version_str = line.split('=')[-1].strip()
                                    if Version(version_str.split('.')[0]) >= self.MIN_JAVA_VERSION:
                                        return Path(exe)
                        except:
                            pass
            except (subprocess.TimeoutExpired, FileNotFoundError):
                continue
        
        return None
    
    def _check_local_java(self) -> Optional[Path]:
        """Check for locally installed Java
        
        Returns:
            Path to local Java executable if found
        """
        java_bin = self.install_dir / "bin"
        
        if sys.platform == "win32":
            java_exe = java_bin / "java.exe"
        else:
            java_exe = java_bin / "java"
        
        if java_exe.exists():
            try:
                result = subprocess.run(
                    [str(java_exe), "-version"],
                    capture_output=True,
                    text=True,
                    timeout=10
                )
                if result.returncode == 0:
                    return java_exe
            except (subprocess.TimeoutExpired, Exception):
                pass
        
        return None
    
    def download_and_install_java(self, progress_callback=None) -> bool:
        """Download and install Java 21
        
        Args:
            progress_callback: Optional callback for download progress (received_bytes, total_bytes)
            
        Returns:
            True if installation successful, False otherwise
        """
        url = self.get_java_download_url()
        
        try:
            response = requests.get(url, stream=True, timeout=300)
            response.raise_for_status()
            
            total_size = int(response.headers.get('content-length', 0))
            
            # Download to temporary file
            with tempfile.NamedTemporaryFile(delete=False, suffix=self._get_archive_extension()) as tmp_file:
                tmp_path = Path(tmp_file.name)
                downloaded = 0
                
                for chunk in response.iter_content(chunk_size=8192):
                    if chunk:
                        tmp_file.write(chunk)
                        downloaded += len(chunk)
                        if progress_callback:
                            progress_callback(downloaded, total_size)
            
            # Extract archive
            extract_dir = self.install_dir / "extracted"
            extract_dir.mkdir(parents=True, exist_ok=True)
            
            if self._extract_archive(tmp_path, extract_dir):
                # Move Java installation to final location
                self._finalize_installation(extract_dir)
                
                # Cleanup
                tmp_path.unlink()
                import shutil
                shutil.rmtree(extract_dir, ignore_errors=True)
                
                return True
            
            # Cleanup on failure
            tmp_path.unlink()
            import shutil
            shutil.rmtree(extract_dir, ignore_errors=True)
            return False
            
        except Exception as e:
            print(f"Error downloading Java: {e}")
            return False
    
    def _get_archive_extension(self) -> str:
        """Get archive extension for current platform
        
        Returns:
            File extension (.zip, .tar.gz, etc.)
        """
        if sys.platform == "win32":
            return ".zip"
        else:
            return ".tar.gz"
    
    def _extract_archive(self, archive_path: Path, extract_dir: Path) -> bool:
        """Extract Java archive
        
        Args:
            archive_path: Path to archive file
            extract_dir: Directory to extract to
            
        Returns:
            True if extraction successful
        """
        import tarfile
        import zipfile
        
        try:
            if archive_path.suffix == ".zip":
                with zipfile.ZipFile(archive_path, 'r') as zip_ref:
                    zip_ref.extractall(extract_dir)
            else:
                with tarfile.open(archive_path, 'r:gz') as tar_ref:
                    tar_ref.extractall(extract_dir)
            return True
        except Exception as e:
            print(f"Error extracting archive: {e}")
            return False
    
    def _finalize_installation(self, extract_dir: Path) -> None:
        """Finalize Java installation by moving files to correct location
        
        Args:
            extract_dir: Directory containing extracted Java files
        """
        import shutil
        
        # Find the JDK directory (usually named jdk-*)
        jdk_dirs = [d for d in extract_dir.iterdir() if d.is_dir() and d.name.startswith("jdk")]
        
        if jdk_dirs:
            jdk_dir = jdk_dirs[0]
            
            # Remove existing installation
            if self.install_dir.exists():
                shutil.rmtree(self.install_dir, ignore_errors=True)
            
            # Move new installation
            shutil.move(str(jdk_dir), str(self.install_dir))
    
    def verify_java(self, java_path: Optional[Path] = None) -> bool:
        """Verify Java installation
        
        Args:
            java_path: Path to Java executable. If None, uses detected path
            
        Returns:
            True if Java is valid and meets version requirements
        """
        if java_path is None:
            java_path = self.java_path
        
        if java_path is None:
            return False
        
        try:
            result = subprocess.run(
                [str(java_path), "-version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            if result.returncode != 0:
                return False
            
            # Check version
            version_output = result.stderr or result.stdout
            if "21" in version_output:
                return True
            
            # Try more detailed version check
            version_result = subprocess.run(
                [str(java_path), "-XshowSettings:properties", "-version"],
                capture_output=True,
                text=True,
                timeout=10
            )
            
            for line in version_result.stderr.split('\n'):
                if 'java.version' in line:
                    version_str = line.split('=')[-1].strip()
                    if Version(version_str.split('.')[0]) >= self.MIN_JAVA_VERSION:
                        return True
            
            return False
            
        except Exception:
            return False
    
    def get_java_path(self) -> Optional[Path]:
        """Get path to Java executable, installing if necessary
        
        Returns:
            Path to Java executable or None if installation failed
        """
        # Check for existing installation
        self.java_path = self.find_installed_java()
        
        if self.java_path and self.verify_java():
            return self.java_path
        
        # Install Java if not found or invalid
        print("Java 21 not found. Downloading and installing...")
        if self.download_and_install_java():
            self.java_path = self._check_local_java()
            if self.java_path and self.verify_java():
                return self.java_path
        
        return None
    
    def get_java_executable(self) -> Optional[str]:
        """Get string path to Java executable
        
        Returns:
            String path to Java executable or None
        """
        java_path = self.get_java_path()
        if java_path:
            return str(java_path)
        return None
