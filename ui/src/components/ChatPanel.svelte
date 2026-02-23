<script>
  let { loadedModels } = $props();

  let selectedModel = $state('');
  let messages = $state([]);
  let input = $state('');
  let generating = $state(false);
  let systemPrompt = $state('You are a helpful AI assistant.');

  // Auto-select first loaded model
  $effect(() => {
    if (!selectedModel && loadedModels.length > 0) {
      selectedModel = loadedModels[0].name;
    }
  });

  async function send() {
    if (!input.trim() || !selectedModel || generating) return;

    const userMsg = { role: 'user', content: input.trim() };
    messages = [...messages, userMsg];
    input = '';
    generating = true;

    const assistantMsg = { role: 'assistant', content: '' };
    messages = [...messages, assistantMsg];

    try {
      const allMessages = [
        { role: 'system', content: systemPrompt },
        ...messages.filter(m => m.content || m.role === 'user').slice(0, -1), // exclude empty assistant
      ];

      const res = await fetch('/v1/chat/completions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({
          model: selectedModel,
          messages: allMessages,
          stream: true,
          max_tokens: 4096,
          temperature: 0.7,
        }),
      });

      const reader = res.body.getReader();
      const decoder = new TextDecoder();
      let buffer = '';

      while (true) {
        const { done, value } = await reader.read();
        if (done) break;

        buffer += decoder.decode(value, { stream: true });
        const lines = buffer.split('\n');
        buffer = lines.pop() || '';

        for (const line of lines) {
          if (line.startsWith('data: ')) {
            const data = line.slice(6).trim();
            if (data === '[DONE]') break;
            try {
              const chunk = JSON.parse(data);
              const delta = chunk.choices?.[0]?.delta?.content;
              if (delta) {
                // Update last message
                messages = messages.map((m, i) =>
                  i === messages.length - 1
                    ? { ...m, content: m.content + delta }
                    : m
                );
              }
            } catch {}
          }
        }
      }
    } catch (e) {
      messages = messages.map((m, i) =>
        i === messages.length - 1
          ? { ...m, content: `Error: ${e.message}` }
          : m
      );
    } finally {
      generating = false;
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
</script>

<div class="chat-container">
  {#if loadedModels.length === 0}
    <div class="no-models">
      <h3>No Models Loaded</h3>
      <p>Load a model from the Models tab to start chatting.</p>
    </div>
  {:else}
    <div class="chat-header">
      <select bind:value={selectedModel}>
        {#each loadedModels as model}
          <option value={model.name}>{model.name}</option>
        {/each}
      </select>
      <button class="clear-btn" onclick={clearChat}>Clear Chat</button>
    </div>

    <div class="messages">
      {#if messages.length === 0}
        <div class="empty-chat">
          <p>Start a conversation with <strong>{selectedModel}</strong></p>
        </div>
      {/if}
      {#each messages as msg}
        <div class="message {msg.role}">
          <div class="role-tag">{msg.role}</div>
          <div class="content">{msg.content}{#if msg.role === 'assistant' && generating && msg === messages[messages.length - 1]}<span class="cursor">▊</span>{/if}</div>
        </div>
      {/each}
    </div>

    <div class="input-area">
      <textarea
        bind:value={input}
        placeholder="Type a message..."
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

  select {
    background: #1a1a2e;
    color: #e0e0e8;
    border: 1px solid #2a2a40;
    padding: 0.4rem 0.8rem;
    border-radius: 6px;
    font-size: 0.9rem;
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
    align-items: center;
    justify-content: center;
    height: 100%;
    color: #444;
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
