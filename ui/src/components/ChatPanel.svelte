<script>
  import { tick, onMount } from 'svelte';

  let { loadedModels } = $props();

  let selectedModel = $state('');
  let messages = $state([]);
  let input = $state('');
  let generating = $state(false);
  let searchStatus = $state('');  // "Searching the web..."
  let systemPrompt = $state(
    `You are a helpful AI assistant running locally via Squig Model Server.\n` +
    `You have access to a web_search tool that lets you search the internet for current information.\n` +
    `When a user asks you to search the web, look something up online, or find current information, use the web_search tool.\n` +
    `When presenting search results, summarize the findings and cite the sources with their URLs.\n` +
    `For things that don't require web search, you can help with reasoning, writing, coding, math, and general knowledge from your training data.\n` +
    `Be honest about your limitations and never fabricate information.`
  );

  /** Web search tool definition (OpenAI-compatible) */
  const WEB_SEARCH_TOOL = {
    type: 'function',
    function: {
      name: 'web_search',
      description: 'Search the web for current information. Use this when the user asks to search, look up, or find information online.',
      parameters: {
        type: 'object',
        properties: {
          query: {
            type: 'string',
            description: 'The search query to look up'
          }
        },
        required: ['query']
      }
    }
  };

  /** Reference to the messages scroll container */
  let messagesEl = $state(null);

  /** Scroll the chat to the bottom */
  function scrollToBottom(smooth = true) {
    if (!messagesEl) return;
    messagesEl.scrollTo({
      top: messagesEl.scrollHeight,
      behavior: smooth ? 'smooth' : 'instant',
    });
  }

  // Auto-select first loaded model
  $effect(() => {
    if (!selectedModel && loadedModels.length > 0) {
      selectedModel = loadedModels[0].name;
    }
  });

  // Auto-scroll when messages change (or search status)
  $effect(() => {
    const _ = messages;
    const __ = searchStatus;
    tick().then(() => scrollToBottom());
  });

  // ─── Tool calling helpers ─────────────────────────────────────────────────

  /** Execute a web_search tool call against our backend */
  async function executeWebSearch(query) {
    const res = await fetch('/api/web-search', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ query, max_results: 5 }),
    });
    return await res.json();
  }

  /** Format search results into text for the tool response message */
  function formatSearchResults(query, data) {
    let text = `Web search results for "${query}":\n\n`;
    if (data.results && data.results.length > 0) {
      data.results.forEach((r, i) => {
        text += `${i + 1}. ${r.title}\n   URL: ${r.url}\n   ${r.snippet}\n\n`;
      });
    } else {
      text += 'No results found.';
      if (data.error) text += ` (${data.error})`;
    }
    return text;
  }

  // ─── Streaming + tool calling flow ────────────────────────────────────────

  /** Stream a chat completion and return { content, toolCalls, finishReason } */
  async function streamCompletion(chatMessages, includeTools = true) {
    const allMessages = [
      { role: 'system', content: systemPrompt },
      ...chatMessages.filter(m => m.content || m.tool_calls || m.tool_call_id),
    ];

    const body = {
      model: selectedModel,
      messages: allMessages,
      stream: true,
      max_tokens: 4096,
      temperature: 0.7,
    };

    if (includeTools) {
      body.tools = [WEB_SEARCH_TOOL];
      body.tool_choice = 'auto';
    }

    const res = await fetch('/v1/chat/completions', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(body),
    });

    if (!res.ok) {
      const err = await res.json().catch(() => ({ error: { message: res.statusText } }));
      throw new Error(err?.error?.message || `HTTP ${res.status}`);
    }

    const reader = res.body.getReader();
    const decoder = new TextDecoder();
    let buffer = '';
    let accumulatedToolCalls = [];
    let finishReason = null;

    while (true) {
      const { done, value } = await reader.read();
      if (done) break;

      buffer += decoder.decode(value, { stream: true });
      const lines = buffer.split('\n');
      buffer = lines.pop() || '';

      for (const line of lines) {
        if (!line.startsWith('data: ')) continue;
        const data = line.slice(6).trim();
        if (data === '[DONE]') continue;

        try {
          const chunk = JSON.parse(data);
          const choice = chunk.choices?.[0];
          if (!choice) continue;

          if (choice.finish_reason) finishReason = choice.finish_reason;
          const delta = choice.delta;
          if (!delta) continue;

          // Content delta → update last message in-place
          if (delta.content) {
            messages = messages.map((m, i) =>
              i === messages.length - 1
                ? { ...m, content: (m.content || '') + delta.content }
                : m
            );
          }

          // Tool call deltas → accumulate
          if (delta.tool_calls) {
            for (const tc of delta.tool_calls) {
              const idx = tc.index ?? 0;
              if (!accumulatedToolCalls[idx]) {
                accumulatedToolCalls[idx] = {
                  id: tc.id || '',
                  type: tc.type || 'function',
                  function: { name: '', arguments: '' },
                };
              }
              if (tc.id) accumulatedToolCalls[idx].id = tc.id;
              if (tc.type) accumulatedToolCalls[idx].type = tc.type;
              if (tc.function?.name) accumulatedToolCalls[idx].function.name += tc.function.name;
              if (tc.function?.arguments) accumulatedToolCalls[idx].function.arguments += tc.function.arguments;
            }
          }
        } catch {}
      }
    }

    return { toolCalls: accumulatedToolCalls.filter(Boolean), finishReason };
  }

  // ─── Main send flow ───────────────────────────────────────────────────────

  async function send() {
    if (!input.trim() || !selectedModel || generating) return;

    const userMsg = { role: 'user', content: input.trim() };
    messages = [...messages, userMsg];
    input = '';
    generating = true;

    try {
      const MAX_TOOL_ROUNDS = 3;  // Prevent infinite loops

      for (let round = 0; round < MAX_TOOL_ROUNDS; round++) {
        // Add empty assistant placeholder
        messages = [...messages, { role: 'assistant', content: '' }];

        // Stream the response (exclude the empty placeholder from the API call)
        const conversationSoFar = messages.slice(0, -1);
        const { toolCalls, finishReason } = await streamCompletion(
          conversationSoFar,
          round < MAX_TOOL_ROUNDS - 1  // Don't offer tools on last round
        );

        // Did the model invoke tools?
        const hasToolCalls = toolCalls.length > 0 &&
          toolCalls.some(tc => tc?.function?.name);

        if (!hasToolCalls) break;  // Normal text response — done

        // Store the tool_calls on the assistant message
        messages = messages.map((m, i) =>
          i === messages.length - 1
            ? { ...m, tool_calls: toolCalls, content: m.content || null }
            : m
        );

        // Execute each tool call
        for (const toolCall of toolCalls) {
          if (toolCall?.function?.name === 'web_search') {
            let query = '';
            try {
              const args = JSON.parse(toolCall.function.arguments);
              query = args.query || toolCall.function.arguments;
            } catch {
              query = toolCall.function.arguments;  // Raw string fallback
            }

            searchStatus = `Searching: "${query}"`;

            try {
              const data = await executeWebSearch(query);
              const resultText = formatSearchResults(query, data);
              messages = [...messages, {
                role: 'tool',
                content: resultText,
                tool_call_id: toolCall.id,
                name: 'web_search',
              }];
            } catch (e) {
              messages = [...messages, {
                role: 'tool',
                content: `Search failed: ${e.message}`,
                tool_call_id: toolCall.id,
                name: 'web_search',
              }];
            }

            searchStatus = '';
          }
        }

        // Loop continues → model will see the tool results and respond
      }
    } catch (e) {
      messages = messages.map((m, i) =>
        i === messages.length - 1
          ? { ...m, content: `Error: ${e.message}` }
          : m
      );
    } finally {
      generating = false;
      searchStatus = '';
    }
  }

  function clearChat() {
    messages = [];
  }

  function handleKeydown(e) {
    if (e.key === 'Enter' && !e.shiftKey) {
      e.preventDefault();
      send();
    }
  }

  // ─── Rendering helpers ────────────────────────────────────────────────────

  /** Lightweight markdown → HTML for chat messages */
  function renderMarkdown(text) {
    if (!text) return '';

    // Escape HTML
    let html = text
      .replace(/&/g, '&amp;')
      .replace(/</g, '&lt;')
      .replace(/>/g, '&gt;');

    // Fenced code blocks: ```lang\n...\n```
    html = html.replace(/```(\w*)\n([\s\S]*?)```/g, (_, lang, code) => {
      const langLabel = lang ? `<span class="code-lang">${lang}</span>` : '';
      return `<div class="code-block">${langLabel}<pre><code>${code.replace(/\n$/, '')}</code></pre></div>`;
    });

    // Inline code: `code`
    html = html.replace(/`([^`\n]+)`/g, '<code class="inline-code">$1</code>');

    // Bold: **text**
    html = html.replace(/\*\*([^*]+)\*\*/g, '<strong>$1</strong>');

    // Italic: *text*
    html = html.replace(/(?<!\*)\*([^*]+)\*(?!\*)/g, '<em>$1</em>');

    // Auto-link URLs (but not inside code blocks/inline code)
    html = html.replace(
      /(?<!href="|src="|<code[^>]*>)(https?:\/\/[^\s<)"'`]+)/g,
      '<a class="chat-link" href="$1" target="_blank" rel="noopener noreferrer">$1</a>'
    );

    // Line breaks (preserve whitespace structure)
    html = html.replace(/\n/g, '<br>');

    return html;
  }

  /** Copy code block to clipboard */
  function copyCode(e) {
    const block = e.target.closest('.code-block');
    if (!block) return;
    const code = block.querySelector('code')?.textContent || '';
    navigator.clipboard.writeText(code);
    e.target.textContent = '✓ Copied';
    setTimeout(() => { e.target.textContent = 'Copy'; }, 1500);
  }

  /**
   * Open external links in the system browser.
   * In Tauri, links don't navigate — we intercept clicks and use the opener plugin.
   * Falls back to window.open for plain browser usage.
   */
  function handleLinkClick(e) {
    const anchor = e.target.closest('a.chat-link');
    if (!anchor) return;

    e.preventDefault();
    e.stopPropagation();
    const url = anchor.getAttribute('href');
    if (!url) return;

    // Try Tauri opener plugin (available when withGlobalTauri: true)
    const tauri = window.__TAURI__;
    if (tauri) {
      // Tauri 2 with opener plugin exposes invoke
      tauri.core.invoke('plugin:opener|open_url', { url }).catch(() => {
        // Last resort
        window.open(url, '_blank');
      });
    } else {
      // Plain browser context
      window.open(url, '_blank', 'noopener,noreferrer');
    }
  }

  // Attach a delegated click handler on the messages container
  onMount(() => {
    // Use capture on document to catch all link clicks in rendered markdown
    document.addEventListener('click', handleLinkClick, true);
    return () => document.removeEventListener('click', handleLinkClick, true);
  });

  /** Check if a message is a visible chat message (not internal tool plumbing) */
  function isVisibleMessage(msg) {
    if (msg.role === 'tool') return false;  // Tool results are internal
    if (msg.role === 'assistant' && msg.tool_calls && !msg.content) return false;  // Pure tool-call msg
    return true;
  }

  /** Extract search query from a tool-call assistant message */
  function getSearchQuery(msg) {
    if (!msg.tool_calls) return '';
    for (const tc of msg.tool_calls) {
      if (tc?.function?.name === 'web_search') {
        try {
          return JSON.parse(tc.function.arguments).query || '';
        } catch {
          return tc.function.arguments || '';
        }
      }
    }
    return '';
  }
</script>

<div class="chat-container">
  {#if loadedModels.length === 0}
    <div class="no-models">
      <h3>No Models Loaded</h3>
      <p>Load a model from the Models tab to start chatting.</p>
    </div>
  {:else}
    <div class="chat-header">
      <div class="header-left">
        <select bind:value={selectedModel}>
          {#each loadedModels as model}
            <option value={model.name}>{model.name}</option>
          {/each}
        </select>
        <span class="web-badge" title="Web search enabled via DuckDuckGo">🔍 Web</span>
      </div>
      <button class="clear-btn" onclick={clearChat}>Clear Chat</button>
    </div>

    <div class="messages" bind:this={messagesEl}>
      {#if messages.length === 0}
        <div class="empty-chat">
          <p>Start a conversation with <strong>{selectedModel}</strong></p>
          <p class="hint">Try: "Search the web for..."</p>
        </div>
      {/if}
      {#each messages as msg, idx}
        {#if msg.role === 'tool'}
          <!-- Hidden: tool results are internal plumbing -->
        {:else if msg.role === 'assistant' && msg.tool_calls && !msg.content}
          <!-- Tool call indicator (searching...) -->
          <div class="message search-indicator">
            <div class="search-icon">🔍</div>
            <div class="search-text">Searched: "{getSearchQuery(msg)}"</div>
          </div>
        {:else}
          <div class="message {msg.role}">
            <div class="role-tag">{msg.role}</div>
            {#if msg.role === 'assistant'}
              <div class="content rendered-md">
                {@html renderMarkdown(msg.content)}
                {#if generating && idx === messages.length - 1}<span class="cursor">▊</span>{/if}
              </div>
            {:else}
              <div class="content">{msg.content}</div>
            {/if}
          </div>
        {/if}
      {/each}

      {#if searchStatus}
        <div class="message search-indicator live">
          <div class="search-icon spin">🔍</div>
          <div class="search-text">{searchStatus}</div>
        </div>
      {/if}
    </div>

    <div class="input-area">
      <textarea
        bind:value={input}
        placeholder="Type a message... (try 'search the web for...')"
        rows="2"
        onkeydown={handleKeydown}
        disabled={generating}
      ></textarea>
      <button class="send-btn" onclick={send} disabled={generating || !input.trim()}>
        {generating ? '...' : 'Send'}
      </button>
    </div>
  {/if}
</div>

<style>
  .chat-container {
    display: flex;
    flex-direction: column;
    height: calc(100vh - 120px);
    background: #0a0a14;
    border: 1px solid #1e1e30;
    border-radius: 10px;
    overflow: hidden;
  }

  .no-models {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #555;
  }
  .no-models h3 { color: #888; margin-bottom: 0.5rem; }

  .chat-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.75rem 1rem;
    background: #0e0e1a;
    border-bottom: 1px solid #1e1e30;
  }

  .header-left {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .web-badge {
    font-size: 0.7rem;
    color: #6ee7b7;
    background: #0e2a20;
    border: 1px solid #1a4030;
    padding: 0.2rem 0.5rem;
    border-radius: 10px;
    white-space: nowrap;
  }

  select {
    background: #1a1a2e;
    color: #e0e0e8;
    border: 1px solid #2a2a40;
    padding: 0.4rem 0.8rem;
    border-radius: 6px;
    font-size: 0.9rem;
  }
  select option {
    background: #1a1a2e;
    color: #e0e0e8;
  }

  .clear-btn {
    background: #1a1a2e;
    color: #888;
    border: 1px solid #2a2a40;
    padding: 0.4rem 0.8rem;
    border-radius: 6px;
    cursor: pointer;
    font-size: 0.85rem;
  }
  .clear-btn:hover { color: #ccc; }

  .messages {
    flex: 1;
    overflow-y: auto;
    padding: 1rem;
  }

  .empty-chat {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #444;
    gap: 0.3rem;
  }
  .hint {
    font-size: 0.8rem;
    color: #555;
    font-style: italic;
  }

  .message {
    margin-bottom: 1rem;
    padding: 0.75rem 1rem;
    border-radius: 8px;
  }
  .message.user {
    background: #1a1a2e;
    margin-left: 2rem;
  }
  .message.assistant {
    background: #0e1a18;
    margin-right: 2rem;
  }
  .message.system {
    background: #1a1a20;
    text-align: center;
    font-size: 0.85rem;
    color: #666;
  }

  /* Search indicator (shown when model calls web_search) */
  .search-indicator {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    background: #0e1a28;
    border: 1px solid #1a3050;
    padding: 0.5rem 1rem;
    border-radius: 8px;
    margin-bottom: 0.75rem;
    font-size: 0.85rem;
    color: #7cb3d0;
  }
  .search-indicator.live {
    border-color: #2a5070;
    animation: pulse-border 1.5s ease-in-out infinite;
  }
  .search-icon { font-size: 1.1rem; }
  .search-icon.spin {
    animation: spin 1.2s linear infinite;
  }
  .search-text {
    font-style: italic;
  }

  @keyframes pulse-border {
    0%, 100% { border-color: #1a3050; }
    50% { border-color: #3a6090; }
  }
  @keyframes spin {
    0% { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
  }

  .role-tag {
    font-size: 0.7rem;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #666;
    margin-bottom: 0.3rem;
  }

  .content {
    white-space: pre-wrap;
    word-break: break-word;
    font-size: 0.95rem;
    line-height: 1.6;
  }

  .rendered-md {
    white-space: normal;
  }

  /* Code blocks */
  .content :global(.code-block) {
    position: relative;
    margin: 0.6rem 0;
    background: #080810;
    border: 1px solid #2a2a40;
    border-radius: 8px;
    overflow: hidden;
  }
  .content :global(.code-block pre) {
    margin: 0;
    padding: 0.8rem 1rem;
    overflow-x: auto;
    white-space: pre;
  }
  .content :global(.code-block code) {
    font-family: 'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace;
    font-size: 0.85rem;
    line-height: 1.5;
    color: #c8d6e5;
  }
  .content :global(.code-lang) {
    display: block;
    padding: 0.25rem 0.75rem;
    background: #0e0e1a;
    border-bottom: 1px solid #2a2a40;
    font-size: 0.7rem;
    color: #6ee7b7;
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  .content :global(.inline-code) {
    background: #1a1a2e;
    border: 1px solid #2a2a40;
    border-radius: 4px;
    padding: 0.1rem 0.35rem;
    font-family: 'JetBrains Mono', 'Fira Code', monospace;
    font-size: 0.85em;
    color: #fbbf24;
  }

  /* Clickable links in chat */
  .content :global(.chat-link) {
    color: #60a5fa;
    text-decoration: underline;
    text-underline-offset: 2px;
    word-break: break-all;
  }
  .content :global(.chat-link:hover) {
    color: #93c5fd;
  }

  .cursor {
    animation: blink 0.8s infinite;
    color: #6ee7b7;
  }
  @keyframes blink {
    50% { opacity: 0; }
  }

  .input-area {
    display: flex;
    gap: 0.5rem;
    padding: 0.75rem 1rem;
    background: #0e0e1a;
    border-top: 1px solid #1e1e30;
  }

  textarea {
    flex: 1;
    background: #1a1a2e;
    color: #e0e0e8;
    border: 1px solid #2a2a40;
    border-radius: 8px;
    padding: 0.6rem 0.8rem;
    font-family: inherit;
    font-size: 0.95rem;
    resize: none;
    outline: none;
  }
  textarea:focus { border-color: #6ee7b7; }

  .send-btn {
    background: #1a3328;
    color: #6ee7b7;
    border: none;
    padding: 0.6rem 1.2rem;
    border-radius: 8px;
    cursor: pointer;
    font-weight: 600;
    font-size: 0.9rem;
    align-self: flex-end;
  }
  .send-btn:hover { background: #1a4030; }
  .send-btn:disabled { opacity: 0.4; cursor: not-allowed; }
</style>
