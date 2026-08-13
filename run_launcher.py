#!/usr/bin/env python3
"""
FenixCraft Launcher - Entry Point Script
Run this script to start the launcher
"""

import sys
from pathlib import Path

# Add parent directory to path for imports
sys.path.insert(0, str(Path(__file__).parent))

from fenixcraft_launcher.main import main

if __name__ == "__main__":
    main()
