"""
Contract Registry for Canonical Oracle Parity Engine
"""

from typing import Dict, Type
from .base import CapabilityContract
from .theme import ThemeContract
from .startup import StartupContract
from .composer import ComposerContract

CONTRACTS: Dict[str, Type[CapabilityContract]] = {
    "theme": ThemeContract,
    "startup": StartupContract,
    "composer": ComposerContract,
}


def get_contract(name: str) -> CapabilityContract:
    if name not in CONTRACTS:
        raise ValueError(f"Unknown contract '{name}'. Available: {list(CONTRACTS.keys())}")
    return CONTRACTS[name]()
