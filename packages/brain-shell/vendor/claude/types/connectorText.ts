export interface ConnectorTextBlock {
  type: 'connector_text';
  text: string;
}

export function isConnectorTextBlock(block: any): block is ConnectorTextBlock {
  return Boolean(block && typeof block === 'object' && block.type === 'connector_text');
}
