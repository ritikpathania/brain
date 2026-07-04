import json
import re

# Synonym mapping to normalize variant names to canonical ID, label, and type
SYNONYM_MAP = {
    # Technologies
    "sqlite": ("sqlite", "SQLite", "technology"),
    "sqlite3": ("sqlite", "SQLite", "technology"),
    "duckdb": ("duckdb", "DuckDB", "technology"),
    "postgres": ("postgresql", "PostgreSQL", "technology"),
    "postgresql": ("postgresql", "PostgreSQL", "technology"),
    "mysql": ("mysql", "MySQL", "technology"),
    "redis": ("redis", "Redis", "technology"),
    "docker": ("docker", "Docker", "technology"),
    "rust": ("rust", "Rust", "language"),
    "python": ("python", "Python", "language"),
    "bun": ("bun", "Bun", "runtime"),
    "react": ("react", "React", "library"),
    
    # Projects
    "brain": ("brain", "Brain", "project"),
    "foodzapp": ("foodzapp", "Foodzapp", "project"),
    
    # People
    "ritikpathania": ("ritikpathania", "ritikpathania", "person"),
    "john": ("john", "John", "person"),
    "alice": ("alice", "Alice", "person"),
    
    # Organizations
    "openai": ("openai", "OpenAI", "organization"),
    "google": ("google", "Google", "organization"),
    
    # Credentials / Configurations / Protocols / Environments
    "api key": ("api-key", "API Key", "credential"),
    "api keys": ("api-key", "API Key", "credential"),
    "api-key": ("api-key", "API Key", "credential"),
    "jwt": ("jwt-token", "JWT Token", "credential"),
    "jwt token": ("jwt-token", "JWT Token", "credential"),
    "jwt tokens": ("jwt-token", "JWT Token", "credential"),
    "database configuration": ("db-config", "Database Configuration", "configuration"),
    "db config": ("db-config", "Database Configuration", "configuration"),
    "environment variables": ("env-vars", "Environment Variables", "environment"),
    "environment variable": ("env-vars", "Environment Variables", "environment"),
    "unix domain socket": ("uds", "Unix Domain Socket", "protocol"),
    "unix domain sockets": ("uds", "Unix Domain Socket", "protocol"),
    "websocket": ("websockets", "WebSockets", "protocol"),
    "websockets": ("websockets", "WebSockets", "protocol"),
}

# Precedence for resolving overlaps: lower is more specific/preferred
TYPE_PRECEDENCE = {
    "technology": 0,
    "project": 0,
    "person": 0,
    "organization": 0,
    "credential": 0,
    "file": 0,
    "language": 0,
    "runtime": 0,
    "library": 0,
    "configuration": 0,
    "environment": 0,
    "protocol": 0,
    "literal": 1,
    "concept": 2,
}


def normalize_entity(text: str):
    """
    Normalizes an entity string to a canonical ID, label, and type.
    """
    text_clean = text.strip()
    text_lower = text_clean.lower()
    
    # 1. Match synonym list
    if text_lower in SYNONYM_MAP:
        return SYNONYM_MAP[text_lower]
        
    # 2. Check if it looks like a file name
    # e.g., config.toml, .env, path/to/file.py
    if re.match(r'^\.?[a-zA-Z0-9_\-\./]+\.[a-zA-Z0-9_]+$', text_clean):
        return text_clean.lower(), text_clean, "file"
        
    # 3. Handle capitalization: proper nouns default to concept (if not matched above)
    if text_clean and text_clean[0].isupper():
        clean_id = re.sub(r'[^a-zA-Z0-9_-]+', '-', text_lower).strip('-')
        return clean_id, text_clean, "concept"
        
def _split_overlapping_proper_nouns(raw_entities, content, stop_words):
    """
    Splits proper noun (concept) matches that contain/overlap with specific entity matches
    (like synonyms, files, literals) to prevent lists of separate items from being grouped.
    """
    concept_entities = [e for e in raw_entities if e["type"] == "concept"]
    specific_entities = [e for e in raw_entities if e["type"] != "concept"]
    processed_entities = list(specific_entities)
    
    for ent in concept_entities:
        start_idx = ent["match_start"]
        end_idx = ent["match_end"]
        
        # Find overlapping specific entities
        overlapping = []
        for spec in specific_entities:
            s_start = spec["match_start"]
            s_end = spec["match_end"]
            if max(start_idx, s_start) < min(end_idx, s_end):
                overlapping.append((max(start_idx, s_start), min(end_idx, s_end)))
        
        if overlapping:
            # Merge overlapping intervals
            overlapping.sort()
            merged = []
            for s, e in overlapping:
                if not merged or merged[-1][1] < s:
                    merged.append((s, e))
                else:
                    merged[-1] = (merged[-1][0], max(merged[-1][1], e))
            
            # Extract non-overlapping sub-spans
            curr = start_idx
            sub_spans = []
            for s, e in merged:
                if s > curr:
                    sub_spans.append((curr, s))
                curr = max(curr, e)
            if curr < end_idx:
                sub_spans.append((curr, end_idx))
            
            # Process each sub-span as a proper noun
            for sub_start, sub_end in sub_spans:
                sub_phrase = content[sub_start:sub_end].strip()
                if not sub_phrase:
                    continue
                
                strip_start = sub_start + (len(content[sub_start:sub_end]) - len(content[sub_start:sub_end].lstrip()))
                strip_end = strip_start + len(sub_phrase)
                
                is_start = False
                if strip_start == 0:
                    is_start = True
                else:
                    pre = content[:strip_start].strip()
                    if not pre or pre[-1] in ('.', '!', '?'):
                        is_start = True
                
                if is_start and ' ' not in sub_phrase:
                    if sub_phrase.lower() in stop_words:
                        continue
                
                pn_id, pn_label, pn_type = normalize_entity(sub_phrase)
                processed_entities.append({
                    "id": pn_id,
                    "label": pn_label,
                    "type": pn_type,
                    "attributes": {},
                    "match_start": strip_start,
                    "match_end": strip_end,
                    "original_text": sub_phrase
                })
        else:
            processed_entities.append(ent)
            
    return processed_entities


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

    # Compile regexes once
    class_pattern = re.compile(
        r"\b([A-Za-z0-9_\-\.]+)\s+(?:is|was|are|were)\s+(?:an?|the)\s+([^.,;:]+)",
        re.IGNORECASE
    )
    
    file_pattern = re.compile(
        r"\b[a-zA-Z0-9_\-/]+\.(toml|env|json|yaml|yml|py|rs|js|ts|txt|db|sqlite)\b|\B\.[eE]nv\b"
    )
    
    prop_noun_pattern = re.compile(
        r"\b[A-Z][a-zA-Z0-9_]*(?:\s+[A-Z][a-zA-Z0-9_]*)*\b"
    )

    # Patterns for relation extraction
    relation_patterns = [
        ("uses", re.compile(r"\b(uses|use|using|utilizes|utilize|utilizing|employs|employ|employing)\b", re.IGNORECASE)),
        ("develops", re.compile(r"\b(develops|develop|developing|developed|built|builds|building|created|creates|creating)\b", re.IGNORECASE)),
        ("stored_in", re.compile(r"\b(stored in|store in|stored_in|kept in|saved in|writes to|write to)\b", re.IGNORECASE)),
        ("runs_on", re.compile(r"\b(runs on|run on|running on|runs_on|hosted on|hosts on|executes on|execute on)\b", re.IGNORECASE)),
        ("depends_on", re.compile(r"\b(depends on|depend on|depending on|depends_on|requires|require|requiring|dependency|dependencies)\b", re.IGNORECASE)),
    ]
    
    passive_relation_patterns = [
        ("uses", re.compile(r"\b(used by|utilised by|employed by)\b", re.IGNORECASE)),
        ("develops", re.compile(r"\b(developed by|built by|created by)\b", re.IGNORECASE)),
        ("runs_on", re.compile(r"\b(run by|hosted by|executed by)\b", re.IGNORECASE)),
        ("depends_on", re.compile(r"\b(depended on by|required by)\b", re.IGNORECASE)),
    ]

    stop_words = {
        "the", "a", "an", "this", "that", "these", "those",
        "it", "they", "he", "she", "we", "you", "i", "our",
        "your", "their", "my", "in", "on", "at", "for", "to",
        "by", "with", "about", "as", "if", "when", "while",
        "there", "here", "all", "any", "both", "each", "few",
        "more", "most", "some", "such", "no", "nor", "not",
        "only", "own", "same", "so", "than", "too", "very",
        "can", "will", "just", "should", "would", "could",
        "and", "but", "or"
    }

    for raw_node in raw_nodes:
        content = raw_node.get("content", "").strip()
        if not content:
            continue

        raw_entities = []

        # 1. Classification clauses property extraction
        for match in class_pattern.finditer(content):
            subject = match.group(1)
            classification = match.group(2).strip()
            
            subj_id, subj_label, subj_type = normalize_entity(subject)
            class_id, class_label, class_type = normalize_entity(classification)
            
            # Subject entity with classification properties
            raw_entities.append({
                "id": subj_id,
                "label": subj_label,
                "type": subj_type,
                "attributes": {"category": classification, "type": classification},
                "match_start": match.start(1),
                "match_end": match.end(1),
                "original_text": subject
            })
            
            # Classification concept entity
            raw_entities.append({
                "id": class_id,
                "label": class_label,
                "type": "concept",
                "attributes": {},
                "match_start": match.start(2),
                "match_end": match.end(2),
                "original_text": classification
            })

            # Explicit classification relation (e.g. Brain is an AI agent engine -> Brain associated_with AI agent engine)
            extracted_edges.append({
                "source": subj_id,
                "target": class_id,
                "relation": "associated_with"
            })

        # 2. File matches
        for match in file_pattern.finditer(content):
            file_name = match.group(0)
            file_id = file_name.lower()
            raw_entities.append({
                "id": file_id,
                "label": file_name,
                "type": "file",
                "attributes": {},
                "match_start": match.start(),
                "match_end": match.end(),
                "original_text": file_name
            })

        # 3. Proper noun matches
        for match in prop_noun_pattern.finditer(content):
            phrase = match.group(0)
            start_idx = match.start()
            
            # Check sentence boundary
            is_start = False
            if start_idx == 0:
                is_start = True
            else:
                pre = content[:start_idx].strip()
                if not pre or pre[-1] in ('.', '!', '?'):
                    is_start = True
            
            if is_start and ' ' not in phrase:
                if phrase.lower() in stop_words:
                    continue
            
            pn_id, pn_label, pn_type = normalize_entity(phrase)
            raw_entities.append({
                "id": pn_id,
                "label": pn_label,
                "type": pn_type,
                "attributes": {},
                "match_start": match.start(),
                "match_end": match.end(),
                "original_text": phrase
            })

        # 4. Known keyword matches from SYNONYM_MAP
        for syn in SYNONYM_MAP.keys():
            escaped = re.escape(syn)
            for match in re.finditer(rf"\b{escaped}\b", content, re.IGNORECASE):
                phrase = match.group(0)
                kw_id, kw_label, kw_type = SYNONYM_MAP[syn]
                raw_entities.append({
                    "id": kw_id,
                    "label": kw_label,
                    "type": kw_type,
                    "attributes": {},
                    "match_start": match.start(),
                    "match_end": match.end(),
                    "original_text": phrase
                })

        # 5. Quoted values as literals
        for match in re.finditer(r"['\"]([^'\"]+)['\"]", content):
            val = match.group(1)
            val_id = val.lower().replace(" ", "-").replace("/", "-")
            raw_entities.append({
                "id": val_id,
                "label": val,
                "type": "literal",
                "attributes": {},
                "match_start": match.start(1),
                "match_end": match.end(1),
                "original_text": val
            })

        # --- Post-pass proper noun splitting stage ---
        raw_entities = _split_overlapping_proper_nouns(raw_entities, content, stop_words)

        # Resolve overlapping entities
        resolved_entities = []
        # Sort by length descending, then by precedence (lower is more specific/preferred)
        sorted_entities = sorted(
            raw_entities,
            key=lambda x: (-(x["match_end"] - x["match_start"]), TYPE_PRECEDENCE.get(x["type"], 99))
        )
        
        for ent in sorted_entities:
            overlap = False
            for kept in resolved_entities:
                if max(ent["match_start"], kept["match_start"]) < min(ent["match_end"], kept["match_end"]):
                    overlap = True
                    break
            if not overlap:
                resolved_entities.append(ent)

        # Sort resolved entities chronologically in the text
        resolved_entities = sorted(resolved_entities, key=lambda x: x["match_start"])

        # Insert resolved entities into the batch of extracted nodes (merging duplicate nodes)
        nodes_in_text = []
        for ent in resolved_entities:
            ent_id = ent["id"]
            if ent_id not in nodes_in_text:
                nodes_in_text.append(ent_id)
            
            if ent_id not in extracted_nodes:
                extracted_nodes[ent_id] = {
                    "id": ent_id,
                    "label": ent["label"],
                    "type": ent["type"],
                    "attributes": ent["attributes"].copy()
                }
            else:
                # Merge logic
                existing = extracted_nodes[ent_id]
                # Keep the more specific type
                if TYPE_PRECEDENCE.get(ent["type"], 99) < TYPE_PRECEDENCE.get(existing["type"], 99):
                    existing["type"] = ent["type"]
                # Update attributes
                existing["attributes"].update(ent["attributes"])

        # 6. Sentence-based relation extraction
        # Split content into sentences
        sentences = re.split(r'(?<=[.!?])\s+', content)
        sent_start_idx = 0
        
        for sentence in sentences:
            sent_end_idx = sent_start_idx + len(sentence)
            
            # Find entities belonging to this sentence
            sent_ents = [
                e for e in resolved_entities 
                if e["match_start"] >= sent_start_idx and e["match_end"] <= sent_end_idx
            ]
            
            # Sort chronologically
            sent_ents = sorted(sent_ents, key=lambda x: x["match_start"])
            
            # Compare every pair of entities in the sentence
            for i in range(len(sent_ents)):
                for j in range(i + 1, len(sent_ents)):
                    e1 = sent_ents[i]
                    e2 = sent_ents[j]
                    
                    e1_rel_start = e1["match_end"] - sent_start_idx
                    e2_rel_start = e2["match_start"] - sent_start_idx
                    
                    # Extract the text between the two entities in this sentence
                    middle_text = sentence[e1_rel_start:e2_rel_start]
                    
                    # Check active relations (e1 -> e2)
                    rel_found = False
                    for rel_name, rel_re in relation_patterns:
                        if rel_re.search(middle_text):
                            extracted_edges.append({
                                "source": e1["id"],
                                "target": e2["id"],
                                "relation": rel_name
                            })
                            rel_found = True
                            break
                            
                    if not rel_found:
                        # Check passive relations
                        for rel_name, rel_re in passive_relation_patterns:
                            if rel_re.search(middle_text):
                                if rel_name == "runs_on":
                                    extracted_edges.append({
                                        "source": e1["id"],
                                        "target": e2["id"],
                                        "relation": "runs_on"
                                    })
                                else:
                                    extracted_edges.append({
                                        "source": e2["id"],
                                        "target": e1["id"],
                                        "relation": rel_name
                                    })
                                rel_found = True
                                break

            sent_start_idx = sent_end_idx + 1

        # 7. Add specific conceptual rule-based extraction to preserve prior deterministic paths
        # Database configs connecting to target engines
        if "db-config" in nodes_in_text:
            for tech in ["sqlite", "postgres", "mysql", "redis", "postgresql"]:
                if tech in nodes_in_text:
                    extracted_edges.append(
                        {
                            "source": "db-config",
                            "target": "sqlite" if tech == "sqlite" else ("postgresql" if tech in ("postgres", "postgresql") else tech),
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

        # 8. Generic relations connecting remaining un-linked nodes sequentially
        if len(resolved_entities) >= 2:
            for i in range(len(resolved_entities) - 1):
                src = resolved_entities[i]["id"]
                tgt = resolved_entities[i + 1]["id"]
                if src == tgt:
                    continue
                already_linked = any(
                    (e["source"] == src and e["target"] == tgt)
                    or (
                        ((e["source"] == src and e["target"] == tgt) or (e["source"] == tgt and e["target"] == src))
                        and e["relation"] != "associated_with"
                    )
                    for e in extracted_edges
                )
                if not already_linked:
                    extracted_edges.append(
                        {"source": src, "target": tgt, "relation": "associated_with"}
                    )

    # De-duplicate edges
    unique_edges = []
    seen_edges = set()
    for edge in extracted_edges:
        edge_key = (edge["source"], edge["target"], edge["relation"])
        if edge_key not in seen_edges:
            seen_edges.add(edge_key)
            unique_edges.append(edge)

    return json.dumps(
        {
            "status": "success",
            "nodes": list(extracted_nodes.values()),
            "edges": unique_edges,
        }
    )
