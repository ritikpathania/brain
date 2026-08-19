"""
Base Classes for Declarative Capability Contracts
"""

from dataclasses import dataclass
from typing import List, Optional, Callable
import pyte


@dataclass
class StageSpec:
    index: int
    name: str
    action_type: str  # 'wait', 'type', 'key', 'type_and_enter', 'assert_disk', 'restart'
    input_bytes: bytes
    settle_time_ms: int
    wait_predicate: Optional[Callable[[pyte.Screen], bool]]
    description: str


class CapabilityContract:
    name: str = "base"
    description: str = "Base capability contract"
    
    def get_stages(self) -> List[StageSpec]:
        raise NotImplementedError("Subclasses must implement get_stages()")
