"""
Main Window for FenixCraft Launcher
PyQt6 GUI interface with fiery theme
"""

import sys
from pathlib import Path
from PyQt6.QtWidgets import (
    QMainWindow, QWidget, QVBoxLayout, QHBoxLayout,
    QLabel, QPushButton, QProgressBar, QTextEdit,
    QFrame, QSizePolicy, QApplication
)
from PyQt6.QtCore import Qt, QThread, pyqtSignal, QTimer
from PyQt6.QtGui import QFont, QColor, QPalette, QLinearGradient, QBrush


class LaunchWorker(QThread):
    """Worker thread for launching Minecraft"""
    
    progress = pyqtSignal(str)
    finished = pyqtSignal(bool)
    
    def __init__(self, launcher):
        super().__init__()
        self.launcher = launcher
    
    def run(self):
        try:
            success = self.launcher.launch(self.progress.emit)
            self.finished.emit(success)
        except Exception as e:
            self.progress.emit(f"Error: {e}")
            self.finished.emit(False)


class MainWindow(QMainWindow):
    """Main window of FenixCraft Launcher"""
    
    def __init__(self, config_manager, java_manager, update_manager, library_manager, minecraft_launcher):
        super().__init__()
        
        self.config = config_manager
        self.java_manager = java_manager
        self.update_manager = update_manager
        self.library_manager = library_manager
        self.minecraft_launcher = minecraft_launcher
        
        self.launch_worker = None
        
        self._setup_ui()
        self._apply_theme()
    
    def _setup_ui(self):
        """Setup user interface"""
        self.setWindowTitle(self.config.launcher_name)
        self.setMinimumSize(800, 600)
        
        # Central widget
        central_widget = QWidget()
        self.setCentralWidget(central_widget)
        
        # Main layout
        main_layout = QVBoxLayout(central_widget)
        main_layout.setContentsMargins(20, 20, 20, 20)
        main_layout.setSpacing(15)
        
        # Header
        header_frame = self._create_header()
        main_layout.addWidget(header_frame)
        
        # Status area
        status_frame = self._create_status_area()
        main_layout.addWidget(status_frame)
        
        # Progress section
        progress_frame = self._create_progress_section()
        main_layout.addWidget(progress_frame)
        
        # Log console
        log_frame = self._create_log_console()
        main_layout.addWidget(log_frame)
        
        # Action buttons
        button_frame = self._create_action_buttons()
        main_layout.addWidget(button_frame)
    
    def _create_header(self) -> QFrame:
        """Create header section"""
        frame = QFrame()
        frame.setFixedHeight(100)
        
        layout = QHBoxLayout(frame)
        layout.setContentsMargins(20, 20, 20, 20)
        
        # Title
        title_label = QLabel("FENIXCRAFT")
        title_font = QFont("Arial", 32, QFont.Weight.Bold)
        title_label.setFont(title_font)
        title_label.setStyleSheet("color: #FF6B35;")
        
        subtitle_label = QLabel("NeoForge 1.21.1 • fenixcraft.ru")
        subtitle_font = QFont("Arial", 12)
        subtitle_label.setFont(subtitle_font)
        subtitle_label.setStyleSheet("color: #E94560;")
        
        # Version badge
        version_label = QLabel(f"v{self.config.launcher_version}")
        version_label.setAlignment(Qt.AlignmentFlag.AlignRight | Qt.AlignmentFlag.AlignVCenter)
        version_label.setStyleSheet("""
            background-color: #E94560;
            color: white;
            padding: 8px 15px;
            border-radius: 15px;
            font-weight: bold;
        """)
        
        layout.addWidget(title_label)
        layout.addStretch()
        layout.addWidget(version_label)
        
        return frame
    
    def _create_status_area(self) -> QFrame:
        """Create status display area"""
        frame = QFrame()
        frame.setStyleSheet("""
            QFrame {
                background-color: rgba(255, 107, 53, 0.1);
                border: 2px solid #FF6B35;
                border-radius: 10px;
            }
        """)
        
        layout = QVBoxLayout(frame)
        layout.setContentsMargins(20, 15, 20, 15)
        
        # Java status
        self.java_status_label = QLabel("Checking Java...")
        self.java_status_label.setStyleSheet("color: #FFA500; font-size: 14px;")
        
        # Mods status
        self.mods_status_label = QLabel("Mods: Not synchronized")
        self.mods_status_label.setStyleSheet("color: #AAAAAA; font-size: 14px;")
        
        layout.addWidget(self.java_status_label)
        layout.addWidget(self.mods_status_label)
        
        return frame
    
    def _create_progress_section(self) -> QFrame:
        """Create progress bar section"""
        frame = QFrame()
        
        layout = QVBoxLayout(frame)
        layout.setContentsMargins(0, 0, 0, 0)
        
        # Progress bar
        self.progress_bar = QProgressBar()
        self.progress_bar.setRange(0, 100)
        self.progress_bar.setValue(0)
        self.progress_bar.setTextVisible(True)
        self.progress_bar.setFormat("%p%")
        self.progress_bar.setStyleSheet("""
            QProgressBar {
                background-color: #1A1A2E;
                border: 2px solid #FF6B35;
                border-radius: 10px;
                text-align: center;
                color: #FFFFFF;
                font-weight: bold;
            }
            QProgressBar::chunk {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #FF6B35, stop:1 #E94560);
                border-radius: 8px;
            }
        """)
        
        # Status label
        self.status_label = QLabel("Ready to launch")
        self.status_label.setAlignment(Qt.AlignmentFlag.AlignCenter)
        self.status_label.setStyleSheet("color: #FFFFFF; font-size: 14px; margin-top: 5px;")
        
        layout.addWidget(self.progress_bar)
        layout.addWidget(self.status_label)
        
        return frame
    
    def _create_log_console(self) -> QFrame:
        """Create log console"""
        frame = QFrame()
        frame.setSizePolicy(QSizePolicy.Policy.Expanding, QSizePolicy.Policy.Expanding)
        
        layout = QVBoxLayout(frame)
        layout.setContentsMargins(0, 0, 0, 0)
        
        # Label
        log_label = QLabel("Console Log:")
        log_label.setStyleSheet("color: #AAAAAA; font-size: 12px;")
        
        # Text edit
        self.log_console = QTextEdit()
        self.log_console.setReadOnly(True)
        self.log_console.setStyleSheet("""
            QTextEdit {
                background-color: #0F0F1E;
                border: 1px solid #FF6B35;
                border-radius: 5px;
                color: #00FF00;
                font-family: 'Consolas', 'Courier New', monospace;
                font-size: 12px;
                padding: 10px;
            }
        """)
        
        layout.addWidget(log_label)
        layout.addWidget(self.log_console)
        
        return frame
    
    def _create_action_buttons(self) -> QFrame:
        """Create action buttons"""
        frame = QFrame()
        frame.setFixedHeight(60)
        
        layout = QHBoxLayout(frame)
        layout.setContentsMargins(0, 0, 0, 0)
        
        # Play button
        self.play_button = QPushButton("▶ PLAY")
        self.play_button.setFont(QFont("Arial", 18, QFont.Weight.Bold))
        self.play_button.setCursor(Qt.CursorShape.PointingHandCursor)
        self.play_button.setStyleSheet("""
            QPushButton {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #FF6B35, stop:1 #E94560);
                color: white;
                border: none;
                border-radius: 25px;
                padding: 15px 50px;
            }
            QPushButton:hover {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #FF8C5A, stop:1 #FF6B85);
            }
            QPushButton:pressed {
                background: qlineargradient(x1:0, y1:0, x2:1, y2:0,
                    stop:0 #E94560, stop:1 #FF6B35);
            }
            QPushButton:disabled {
                background: #555555;
                color: #888888;
            }
        """)
        self.play_button.clicked.connect(self._on_play_clicked)
        
        # Settings button
        settings_button = QPushButton("⚙ Settings")
        settings_button.setFont(QFont("Arial", 12))
        settings_button.setCursor(Qt.CursorShape.PointingHandCursor)
        settings_button.setStyleSheet("""
            QPushButton {
                background-color: #1A1A2E;
                color: #FF6B35;
                border: 2px solid #FF6B35;
                border-radius: 20px;
                padding: 10px 25px;
            }
            QPushButton:hover {
                background-color: #FF6B35;
                color: white;
            }
        """)
        
        layout.addStretch()
        layout.addWidget(settings_button)
        layout.addWidget(self.play_button)
        layout.addStretch()
        
        return frame
    
    def _apply_theme(self):
        """Apply fiery theme to the application"""
        self.setStyleSheet("""
            QMainWindow {
                background: qlineargradient(x1:0, y1:0, x2:0, y2:1,
                    stop:0 #0F0F1E, stop:1 #1A1A2E);
            }
            QLabel {
                color: #FFFFFF;
            }
        """)
    
    def _log(self, message: str):
        """Add message to log console"""
        from datetime import datetime
        timestamp = datetime.now().strftime("%H:%M:%S")
        self.log_console.append(f"[{timestamp}] {message}")
        self.log_console.verticalScrollBar().setValue(
            self.log_console.verticalScrollBar().maximum()
        )
    
    def _on_play_clicked(self):
        """Handle play button click"""
        self.play_button.setEnabled(False)
        self._log("Starting launch sequence...")
        
        # Start launch worker
        self.launch_worker = LaunchWorker(self.minecraft_launcher)
        self.launch_worker.progress.connect(self._on_launch_progress)
        self.launch_worker.finished.connect(self._on_launch_finished)
        self.launch_worker.start()
    
    def _on_launch_progress(self, message: str):
        """Handle launch progress update"""
        self._log(message)
        self.status_label.setText(message)
    
    def _on_launch_finished(self, success: bool):
        """Handle launch completion"""
        if success:
            self._log("Game launched successfully!")
            self.status_label.setText("Game running")
        else:
            self._log("Failed to launch game")
            self.status_label.setText("Launch failed")
        
        self.play_button.setEnabled(True)
        self.launch_worker = None
