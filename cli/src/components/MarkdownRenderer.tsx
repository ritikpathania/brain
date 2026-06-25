import React from 'react';
import { ASTNode } from '../services/MarkdownParser';
import { ThemedBox, ThemedText, Divider } from './design-system';

// Compile-time exhaustiveness checker
function assertNever(x: never): never {
  const node = x as any;
  const typeStr = node && typeof node === 'object' && 'type' in node ? node.type : typeof node;
  throw new Error(`Unhandled AST node type "${typeStr}". Full node: ${JSON.stringify(node)}`);
}

export const MarkdownRenderer: React.FC<{ nodes: ASTNode[] }> = ({ nodes }) => {
  return (
    <ThemedBox flexDirection="column" width="100%">
      {nodes.map((node, index) => (
        <ThemedBox key={index} flexDirection="column" marginBottom={1}>
          {renderNode(node)}
        </ThemedBox>
      ))}
    </ThemedBox>
  );
};

export function renderNode(node: ASTNode): React.ReactNode {
  switch (node.type) {
    case 'paragraph':
      return (
        <ThemedBox flexDirection="row" flexWrap="wrap">
          {node.children.map((child, idx) => (
            <React.Fragment key={idx}>{renderInlineNode(child)}</React.Fragment>
          ))}
        </ThemedBox>
      );

    case 'heading':
      let color = 'text';
      if (node.level === 1) color = 'claude';
      else if (node.level === 2) color = 'primary';
      else color = 'suggestion';

      return (
        <ThemedBox flexDirection="row" marginTop={1}>
          <ThemedText color={color} bold>
            {'#'.repeat(node.level)} 
          </ThemedText>
          <ThemedBox flexDirection="row" marginLeft={1}>
            {node.children.map((child, idx) => (
              <React.Fragment key={idx}>{renderInlineNode(child)}</React.Fragment>
            ))}
          </ThemedBox>
        </ThemedBox>
      );

    case 'list':
      return (
        <ThemedBox flexDirection="column" marginLeft={2}>
          {node.children.map((child, idx) => {
            if (child.type === 'listitem') {
              const prefix = node.ordered ? `${idx + 1}. ` : '• ';
              return (
                <ThemedBox key={idx} flexDirection="row">
                  <ThemedText color="claude" bold>
                    {prefix}
                  </ThemedText>
                  <ThemedBox flexDirection="row" flexWrap="wrap">
                    {child.children.map((inlineChild, cIdx) => (
                      <React.Fragment key={cIdx}>{renderInlineNode(inlineChild)}</React.Fragment>
                    ))}
                  </ThemedBox>
                </ThemedBox>
              );
            }
            return renderNode(child);
          })}
        </ThemedBox>
      );

    case 'listitem':
      // This is usually handled inside parent 'list', but fallback if rendered directly
      return (
        <ThemedBox flexDirection="row">
          <ThemedText color="claude" bold>• </ThemedText>
          <ThemedBox flexDirection="row" flexWrap="wrap">
            {node.children.map((child, idx) => (
              <React.Fragment key={idx}>{renderInlineNode(child)}</React.Fragment>
            ))}
          </ThemedBox>
        </ThemedBox>
      );

    case 'codeblock':
      return (
        <ThemedBox
          flexDirection="column"
          borderStyle="single"
          borderColor="promptBorder"
          backgroundColor="messageActionsBackground"
          padding={1}
          marginY={1}
          width="100%"
        >
          {node.lang && (
            <ThemedBox marginBottom={1} borderStyle="classic" borderColor="subtle" paddingBottom={0}>
              <ThemedText color="inactive" bold italic>
                // Language: {node.lang}
              </ThemedText>
            </ThemedBox>
          )}
          {renderSyntaxHighlightedCode(node.code)}
        </ThemedBox>
      );

    case 'horizontalrule':
      return <Divider color="subtle" />;

    case 'quote':
      return (
        <ThemedBox
          flexDirection="row"
          borderStyle="classic"
          borderColor="claude"
          paddingLeft={1}
          marginLeft={1}
          marginY={1}
        >
          <ThemedBox flexDirection="column" width="100%">
            {node.children.map((child, idx) => (
              <React.Fragment key={idx}>{renderNode(child)}</React.Fragment>
            ))}
          </ThemedBox>
        </ThemedBox>
      );

    case 'bold':
    case 'italic':
    case 'text':
    case 'inlinecode':
      // Direct blocks fallback to inline mapping
      return renderInlineNode(node);

    default:
      return assertNever(node);
  }
}

function renderInlineNode(node: ASTNode): React.ReactNode {
  switch (node.type) {
    case 'text':
      return <ThemedText color="text">{node.value}</ThemedText>;

    case 'bold':
      return (
        <ThemedText color="primary" bold>
          {node.text}
        </ThemedText>
      );

    case 'italic':
      return (
        <ThemedText color="suggestion" underline>
          {node.text}
        </ThemedText>
      );

    case 'inlinecode':
      return (
        <ThemedText color="chromeYellow" bold>
          {` ${node.code} `}
        </ThemedText>
      );

    case 'paragraph':
    case 'heading':
    case 'list':
    case 'listitem':
    case 'codeblock':
    case 'horizontalrule':
    case 'quote':
      // Non-inline blocks processed normally
      return renderNode(node);

    default:
      return assertNever(node);
  }
}

function renderSyntaxHighlightedCode(code: string): React.ReactNode {
  const lines = code.split('\n');
  const keywords = new Set([
    'let', 'const', 'var', 'fn', 'function', 'import', 'export', 'from',
    'return', 'struct', 'impl', 'pub', 'use', 'class', 'interface',
    'type', 'default', 'case', 'switch', 'for', 'while', 'if', 'else',
  ]);

  return (
    <ThemedBox flexDirection="column">
      {lines.map((line, lineIdx) => {
        // Comment check
        const trimmed = line.trim();
        if (trimmed.startsWith('//') || trimmed.startsWith('#') || trimmed.startsWith('/*')) {
          return (
            <ThemedText key={lineIdx} color="inactive">
              {line}
            </ThemedText>
          );
        }

        // Tokenize line words to highlight keywords
        const tokens = line.split(/(\s+|\W)/);
        return (
          <ThemedBox key={lineIdx} flexDirection="row" flexWrap="wrap">
            {tokens.map((token, tokenIdx) => {
              if (keywords.has(token)) {
                return (
                  <ThemedText key={tokenIdx} color="professionalBlue" bold>
                    {token}
                  </ThemedText>
                );
              }
              if (token.startsWith('"') || token.startsWith("'")) {
                return (
                  <ThemedText key={tokenIdx} color="success">
                    {token}
                  </ThemedText>
                );
              }
              if (/^\d+$/.test(token)) {
                return (
                  <ThemedText key={tokenIdx} color="chromeYellow">
                    {token}
                  </ThemedText>
                );
              }
              return (
                <ThemedText key={tokenIdx} color="text">
                  {token}
                </ThemedText>
              );
            })}
          </ThemedBox>
        );
      })}
    </ThemedBox>
  );
}
