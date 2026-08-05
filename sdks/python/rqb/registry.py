#!/usr/bin/env python3
"""
Dynamic VectorRegistry for Data-Driven RQB Evaluators
"""

import importlib
import os
import sys
from typing import Dict, Type, List
from engine import Vector

class VectorRegistry:
    _registry: Dict[int, Type[Vector]] = {}

    @classmethod
    def register(cls, vector_id: int):
        def decorator(evaluator_cls: Type[Vector]):
            cls._registry[vector_id] = evaluator_cls
            return evaluator_cls
        return decorator

    @classmethod
    def get_evaluators(cls) -> List[Vector]:
        evaluators = []
        for vid in sorted(cls._registry.keys()):
            evaluators.append(cls._registry[vid]())
        return evaluators

    @classmethod
    def load_vector_modules(cls, vectors_dir: str):
        sys.path.insert(0, vectors_dir)
        for fname in os.listdir(vectors_dir):
            if fname.endswith(".py") and not fname.startswith("__"):
                mod_name = fname[:-3]
                importlib.import_module(mod_name)
