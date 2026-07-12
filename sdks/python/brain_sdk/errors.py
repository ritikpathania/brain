"""Exception definitions for the Brain SDK."""

class BrainSdkError(Exception):
    """Base exception class for all Brain SDK errors."""
    pass

class BrainConnectionError(BrainSdkError):
    """Raised when connecting, reading, or writing to the daemon's UDS socket fails."""
    pass

class BrainProtocolError(BrainSdkError):
    """Raised when message framing or serialization invariants are violated."""
    pass

class BrainTimeoutError(BrainSdkError):
    """Raised when the daemon fails to reply within the configured timeout."""
    pass

class BrainDaemonError(BrainSdkError):
    """Raised when the daemon returns a structured error response."""
    pass
