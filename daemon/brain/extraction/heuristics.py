import json
import re


def extract_semantic_nodes(json_input: str) -> str:
    """
    FFI entrypoint called by the Rust daemon.
    Takes a serialized JSON array of raw STM nodes, runs NLP/regex heuristics,
    and returns a serialized JSON graph of nodes and edges.
    """
    try:
        raw_nodes = json.loads(json_input)
    except Exception as e:
        return json.dumps(
            {
                "status": "error",
                "message": f"FFI failed to decode input: {e}",
                "nodes": [],
                "edges": [],
            }
        )

    extracted_nodes = {}
    extracted_edges = []

    # Deterministic rule-based entity extractor
    for raw_node in raw_nodes:
        content = raw_node.get("content", "").strip()
        if not content:
            continue

        nodes_in_text = []

        # 1. Regex Technology Matches
        tech_patterns = {
            r"\bsqlite\b": ("sqlite", "SQLite", "technology", {"engine": "SQLite"}),
            r"\bpostgres(ql)?\b": (
                "postgres",
                "PostgreSQL",
                "technology",
                {"engine": "PostgreSQL"},
            ),
            r"\bmysql\b": ("mysql", "MySQL", "technology", {"engine": "MySQL"}),
            r"\bredis\b": ("redis", "Redis", "technology", {"engine": "Redis"}),
            r"\bdocker\b": ("docker", "Docker", "technology", {"runtime": "Docker"}),
            r"\brust\b": ("rust", "Rust", "language", {}),
            r"\bpython\b": ("python", "Python", "language", {}),
            r"\bbun\b": ("bun", "Bun", "runtime", {}),
            r"\breact\b": ("react", "React", "library", {}),
        }
        for pattern, (node_id, label, n_type, attrs) in tech_patterns.items():
            if re.search(pattern, content, re.IGNORECASE):
                extracted_nodes[node_id] = {
                    "id": node_id,
                    "label": label,
                    "type": n_type,
                    "attributes": attrs,
                }
                nodes_in_text.append(node_id)

        # 2. Regex Configuration & Concept Matches
        concept_patterns = {
            r"\b(database configuration|db config)\b": (
                "db-config",
                "Database Configuration",
                "configuration",
                {},
            ),
            r"\bapi\s*key(s)?\b": ("api-key", "API Key", "credential", {}),
            r"\benvironment variables?\b": (
                "env-vars",
                "Environment Variables",
                "environment",
                {},
            ),
            r"\bunix domain socket(s)?\b": (
                "uds",
                "Unix Domain Socket",
                "protocol",
                {},
            ),
            r"\bwebsocket(s)?\b": ("websockets", "WebSockets", "protocol", {}),
        }
        for pattern, (node_id, label, n_type, attrs) in concept_patterns.items():
            if re.search(pattern, content, re.IGNORECASE):
                extracted_nodes[node_id] = {
                    "id": node_id,
                    "label": label,
                    "type": n_type,
                    "attributes": attrs,
                }
                nodes_in_text.append(node_id)

        # 3. Quoted Substrings Fallback Extraction (e.g. files, paths, settings)
        quoted_values = re.findall(r"['\"]([^'\"]+)['\"]", content)
        for val in quoted_values:
            val_id = val.lower().replace(" ", "-").replace("/", "-")
            extracted_nodes[val_id] = {
                "id": val_id,
                "label": val,
                "type": "literal",
                "attributes": {},
            }
            nodes_in_text.append(val_id)

        # 4. Relations Mapping Heuristics
        # Database configs connecting to target engines
        if "db-config" in nodes_in_text:
            for tech in ["sqlite", "postgres", "mysql", "redis"]:
                if tech in nodes_in_text:
                    extracted_edges.append(
                        {
                            "source": "db-config",
                            "target": tech,
                            "relation": "configures",
                        }
                    )

        # Credentials stored in environment variables
        if "api-key" in nodes_in_text and "env-vars" in nodes_in_text:
            extracted_edges.append(
                {"source": "api-key", "target": "env-vars", "relation": "stored_in"}
            )

        # Runtime protocols
        if "uds" in nodes_in_text or "websockets" in nodes_in_text:
            proto = "uds" if "uds" in nodes_in_text else "websockets"
            for tech in ["bun", "rust", "python"]:
                if tech in nodes_in_text:
                    extracted_edges.append(
                        {
                            "source": tech,
                            "target": proto,
                            "relation": "communicates_via",
                        }
                    )

        # Generic relations connecting remaining un-linked nodes sequentially
        if len(nodes_in_text) >= 2:
            for i in range(len(nodes_in_text) - 1):
                src = nodes_in_text[i]
                tgt = nodes_in_text[i + 1]
                # Check if this edge is already defined to avoid duplicate edges
                already_linked = any(
                    (e["source"] == src and e["target"] == tgt)
                    or (e["source"] == tgt and e["target"] == src)
                    for e in extracted_edges
                )
                if not already_linked:
                    extracted_edges.append(
                        {"source": src, "target": tgt, "relation": "associated_with"}
                    )

    return json.dumps(
        {
            "status": "success",
            "nodes": list(extracted_nodes.values()),
            "edges": extracted_edges,
        }
    )
