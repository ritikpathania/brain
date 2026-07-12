export class BrainError extends Error {
    constructor(message: string) {
        super(message);
        this.name = 'BrainError';
        Object.setPrototypeOf(this, new.target.prototype);
    }
}

export class TransportError extends BrainError {
    constructor(message: string, public readonly cause?: Error) {
        super(message);
        this.name = 'TransportError';
    }
}
