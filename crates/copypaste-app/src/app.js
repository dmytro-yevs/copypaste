// Tauri 2 global APIs (available because withGlobalTauri: true)
const invoke = window.__TAURI__.core.invoke;
const writeText = window.__TAURI__.clipboardManager.writeText;
const getCurrentWindow = window.__TAURI__.window.getCurrentWindow;

let debounceTimer = null;
let currentItems = [];

const searchInput = document.getElementById('search-input');
const itemList = document.getElementById('item-list');
const statusText = document.getElementById('status-text');

// --- Initialization ---

document.addEventListener('DOMContentLoaded', () => {
  loadItems();
  searchInput.focus();

  searchInput.addEventListener('input', () => {
    clearTimeout(debounceTimer);
    debounceTimer = setTimeout(() => {
      const q = searchInput.value.trim();
      if (q.length === 0) loadItems();
      else searchItems(q);
    }, 200);
  });

  document.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') getCurrentWindow().hide();
  });
});

// Reload items each time the window gains focus
window.addEventListener('focus', () => {
  const q = searchInput.value.trim();
  if (q.length === 0) loadItems();
  else searchItems(q);
});

// --- Data fetching ---

async function loadItems() {
  try {
    const result = await invoke('list_items', { limit: 20 });
    currentItems = result.items;
    renderItems(result.items, result.total);
  } catch (err) {
    renderError(String(err.message || err));
  }
}

async function searchItems(query) {
  try {
    const result = await invoke('search_items', { query, limit: 20 });
    currentItems = result.items;
    renderItems(result.items, result.total);
  } catch (err) {
    renderError(String(err.message || err));
  }
}

async function deleteItem(id) {
  try {
    await invoke('delete_item', { id });
    const q = searchInput.value.trim();
    if (q.length === 0) loadItems();
    else searchItems(q);
  } catch (err) {
    console.error('Delete failed:', String(err.message || err));
  }
}

async function copyItem(item) {
  try {
    await writeText(item.snippet);
    getCurrentWindow().hide();
  } catch (err) {
    console.error('Copy failed:', String(err.message || err));
  }
}

// --- Rendering (DOM-only, no innerHTML with untrusted content) ---

function renderItems(items, total) {
  statusText.textContent = total + ' item' + (total !== 1 ? 's' : '');
  itemList.textContent = '';  // clear safely

  if (items.length === 0) {
    const msg = document.createElement('div');
    msg.className = 'empty-state';
    const span = document.createElement('span');
    span.textContent = searchInput.value.trim() ? 'No results' : 'Clipboard is empty';
    msg.appendChild(span);
    itemList.appendChild(msg);
    return;
  }

  const fragment = document.createDocumentFragment();
  items.forEach(item => {
    fragment.appendChild(buildItemRow(item));
  });
  itemList.appendChild(fragment);

  // Event delegation — one handler for the whole list
  itemList.onclick = (e) => {
    const deleteBtn = e.target.closest('.btn-delete');
    if (deleteBtn) {
      e.stopPropagation();
      deleteItem(deleteBtn.dataset.id);
      return;
    }
    const row = e.target.closest('.clip-item');
    if (row) {
      const item = currentItems.find(i => i.id === row.dataset.id);
      if (item) copyItem(item);
    }
  };
}

function buildItemRow(item) {
  const row = document.createElement('div');
  row.className = 'clip-item';
  row.dataset.id = item.id;

  const content = document.createElement('div');
  content.className = 'clip-content';

  const snippet = document.createElement('div');
  snippet.className = 'clip-snippet' + (item.is_sensitive ? ' is-sensitive' : '');
  // textContent prevents XSS for all user-supplied clipboard content
  snippet.textContent = item.is_sensitive ? '[sensitive]' : item.snippet;

  const meta = document.createElement('div');
  meta.className = 'clip-meta';
  meta.textContent = formatTime(item.wall_time);

  content.appendChild(snippet);
  content.appendChild(meta);

  const typeLabel = document.createElement('span');
  typeLabel.className = 'clip-type';
  typeLabel.textContent = item.content_type;

  const deleteBtn = document.createElement('button');
  deleteBtn.className = 'btn-delete';
  deleteBtn.dataset.id = item.id;
  deleteBtn.title = 'Delete';
  deleteBtn.textContent = '×';  // multiplication sign (x)

  row.appendChild(content);
  row.appendChild(typeLabel);
  row.appendChild(deleteBtn);
  return row;
}

function renderError(message) {
  itemList.textContent = '';
  const div = document.createElement('div');
  div.className = 'empty-state is-error';
  const span = document.createElement('span');
  span.textContent = message;
  div.appendChild(span);
  itemList.appendChild(div);
  statusText.textContent = 'Error';
}

// --- Utilities ---

function formatTime(wallTimeMs) {
  if (!wallTimeMs) return '';
  const d = new Date(wallTimeMs);
  const diffSec = Math.floor((Date.now() - d.getTime()) / 1000);
  if (diffSec < 60) return 'just now';
  if (diffSec < 3600) return Math.floor(diffSec / 60) + 'm ago';
  if (diffSec < 86400) return Math.floor(diffSec / 3600) + 'h ago';
  return d.toLocaleDateString();
}
