"""
Canonical Claude Oracle Parity Engine Package
"""

from .terminal import CanonicalCell, CanonicalFrame
from .comparator import StageDiff, diff_exact_grid_frames
from .process import OracleSession
from .contracts import get_contract
