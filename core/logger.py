"""Logger rotativo compartido por toda la app (logs/clipvault.log)."""
import os
import logging
from logging.handlers import RotatingFileHandler

from core.config import LOG_DIR

os.makedirs(LOG_DIR, exist_ok=True)

logger = logging.getLogger("clipvault")
logger.setLevel(logging.INFO)
_handler = RotatingFileHandler(os.path.join(LOG_DIR, "clipvault.log"), maxBytes=2_000_000, backupCount=3, encoding="utf-8")
_handler.setFormatter(logging.Formatter("%(asctime)s [%(levelname)s] %(message)s"))
logger.addHandler(_handler)
