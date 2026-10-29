// ==========================================================================
// GNOSIS PWA — CLIENT RUNTIME & OFFLINE-FIRST LOGIC
// ==========================================================================

const GNOSIS_DB_NAME = 'gnosis_vault_db';
const GNOSIS_STORE_NAME = 'notes';

class GnosisApp {
  constructor() {
    this.db = null;
    this.currentNoteId = null;
    this.notes = [];
    this.themeList = ['Abyss', 'Aether', 'Arcana', 'Boreas'];
    this.currentThemeIndex = 0;
    this.deferredPrompt = null;
    this.graphSimulation = null;

    this.initElements();
    this.initServiceWorker();
    this.initIndexedDb().then(() => {
      this.loadNotes();
    });
    this.bindEvents();
    this.initGraph();
  }

  initElements() {
    this.btnInstall = document.getElementById('btn-install');
    this.noteListEl = document.getElementById('note-list');
    this.noteTitleInput = document.getElementById('note-title-input');
    this.markdownEditor = document.getElementById('markdown-editor');
    this.statusWords = document.getElementById('status-words');
    this.statusChars = document.getElementById('status-chars');
    this.statusPlugin = document.getElementById('status-plugin');
    this.themeBtn = document.getElementById('btn-theme');
    this.quickSwitcherModal = document.getElementById('quick-switcher-modal');
    this.qsInput = document.getElementById('qs-input');
    this.qsResults = document.getElementById('qs-results');
    this.editorPane = document.getElementById('editor-pane');
    this.graphPane = document.getElementById('graph-pane');
    this.tabEditor = document.getElementById('tab-editor');
    this.tabGraph = document.getElementById('tab-graph');
    this.graphCanvas = document.getElementById('graph-canvas');
  }

  // 1. Registro do Service Worker PWA para suporte 100% offline
  async initServiceWorker() {
    if ('serviceWorker' in navigator) {
      try {
        const registration = await navigator.serviceWorker.register('./sw.js', { scope: './' });
        console.log('[Gnosis PWA] Service Worker registrado com sucesso:', registration.scope);
      } catch (err) {
        console.warn('[Gnosis PWA] Falha ao registrar Service Worker:', err);
      }
    }

    // Suporte ao botão "Instalar PWA" no navegador
    window.addEventListener('beforeinstallprompt', (e) => {
      e.preventDefault();
      this.deferredPrompt = e;
      if (this.btnInstall) {
        this.btnInstall.style.display = 'inline-flex';
      }
    });

    window.addEventListener('appinstalled', () => {
      console.log('[Gnosis PWA] Aplicativo instalado com sucesso no sistema');
      if (this.btnInstall) {
        this.btnInstall.style.display = 'none';
      }
      this.deferredPrompt = null;
    });
  }

  // 2. Inicialização do Armazenamento Local IndexedDB (VFS Web)
  initIndexedDb() {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open(GNOSIS_DB_NAME, 1);

      request.onupgradeneeded = (e) => {
        const db = e.target.result;
        if (!db.objectStoreNames.contains(GNOSIS_STORE_NAME)) {
          const store = db.createObjectStore(GNOSIS_STORE_NAME, { keyPath: 'id' });
          store.createIndex('title', 'title', { unique: false });
          store.createIndex('updatedAt', 'updatedAt', { unique: false });
        }
      };

      request.onsuccess = (e) => {
        this.db = e.target.result;
        resolve(this.db);
      };

      request.onerror = (e) => {
        console.error('[Gnosis DB] Erro ao abrir IndexedDB:', e);
        reject(e);
      };
    });
  }

  // 3. Carga e persistência de notas no cliente
  async loadNotes() {
    const tx = this.db.transaction(GNOSIS_STORE_NAME, 'readonly');
    const store = tx.objectStore(GNOSIS_STORE_NAME);
    const request = store.getAll();

    request.onsuccess = () => {
      this.notes = request.result || [];
      if (this.notes.length === 0) {
        // Notas iniciais demonstrativas
        this.createInitialNotes();
      } else {
        this.renderNoteList();
        this.openNote(this.notes[0].id);
      }
    };
  }

  async createInitialNotes() {
    const welcomeNote = {
      id: 'welcome',
      title: 'Boas-vindas ao Gnosis Web',
      sticker: '🌌',
      content: `# Boas-vindas ao Gnosis (PWA Offline-First)\n\nO **Gnosis** é o seu segundo cérebro soberano e estritamente local, rodando 100% no seu navegador sem necessitar de servidores em nuvem.\n\n### ⚡ Recursos Principais:\n- **Modo Avião Nativo:** Abra em qualquer lugar, mesmo sem internet.\n- **Quick Switcher:** Pressione \`Ctrl+O\` para buscar qualquer nota.\n- **Grafo de Conhecimento:** Visualize conexões no Canvas 2D.\n- **Privacidade Soberana:** Suas notas residem no VFS Web local.\n\n[[Arquitetura]] | [[Plugins]]`,
      updatedAt: Date.now()
    };

    const archNote = {
      id: 'architecture',
      title: 'Arquitetura do Gnosis',
      sticker: '🧠',
      content: `# Arquitetura do Gnosis\n\nConstruído com **Rust Seguro** e renderização de alto desempenho.\n\n- [[Boas-vindas ao Gnosis Web]]\n- [[Grafo de Conhecimento]]`,
      updatedAt: Date.now()
    };

    const graphNote = {
      id: 'graph-note',
      title: 'Grafo de Conhecimento',
      sticker: '🕸️',
      content: `# Grafo Interativo 2D\n\nSimulação física baseada em nós e arestas conectados por wikilinks.\n\nVeja a aba Grafo para interagir.`,
      updatedAt: Date.now()
    };

    await this.saveNoteToDb(welcomeNote);
    await this.saveNoteToDb(archNote);
    await this.saveNoteToDb(graphNote);

    this.notes = [welcomeNote, archNote, graphNote];
    this.renderNoteList();
    this.openNote(welcomeNote.id);
  }

  saveNoteToDb(note) {
    return new Promise((resolve, reject) => {
      const tx = this.db.transaction(GNOSIS_STORE_NAME, 'readwrite');
      const store = tx.objectStore(GNOSIS_STORE_NAME);
      const req = store.put(note);
      req.onsuccess = () => resolve();
      req.onerror = (e) => reject(e);
    });
  }

  renderNoteList() {
    this.noteListEl.innerHTML = '';
    this.notes.forEach((note) => {
      const li = document.createElement('li');
      li.className = `note-item ${note.id === this.currentNoteId ? 'active' : ''}`;
      li.innerHTML = `
        <span class="note-sticker">${note.sticker || '📄'}</span>
        <span class="note-name">${note.title}</span>
      `;
      li.addEventListener('click', () => this.openNote(note.id));
      this.noteListEl.appendChild(li);
    });
  }

  openNote(noteId) {
    const note = this.notes.find((n) => n.id === noteId);
    if (!note) return;

    this.currentNoteId = note.id;
    this.noteTitleInput.value = note.title;
    this.markdownEditor.value = note.content;
    this.updateCounts();
    this.renderNoteList();
    this.updateGraphSimulation();
  }

  saveCurrentNote() {
    if (!this.currentNoteId) return;
    const note = this.notes.find((n) => n.id === this.currentNoteId);
    if (!note) return;

    note.title = this.noteTitleInput.value.trim() || 'Sem Título';
    note.content = this.markdownEditor.value;
    note.updatedAt = Date.now();

    this.saveNoteToDb(note);
    this.updateCounts();
    this.renderNoteList();
    this.updateGraphSimulation();
  }

  createNewNote() {
    const newId = 'note_' + Date.now();
    const newNote = {
      id: newId,
      title: 'Nova Nota',
      sticker: '✨',
      content: '# Nova Nota\n\nComece a registrar seus pensamentos aqui...',
      updatedAt: Date.now()
    };
    this.notes.unshift(newNote);
    this.saveNoteToDb(newNote);
    this.openNote(newId);
  }

  updateCounts() {
    const text = this.markdownEditor.value;
    const words = text.trim() ? text.trim().split(/\s+/).length : 0;
    const chars = text.length;

    this.statusWords.textContent = `${words} palavras`;
    this.statusChars.textContent = `${chars} caracteres`;
    this.statusPlugin.textContent = `📊 Contador Ativo: ${words} pal.`;
  }

  // 4. Alternância Dinâmica de Temas Místicos
  cycleTheme() {
    this.currentThemeIndex = (this.currentThemeIndex + 1) % this.themeList.length;
    const nextTheme = this.themeList[this.currentThemeIndex];
    document.body.setAttribute('data-theme', nextTheme);
    this.themeBtn.textContent = `🎨 ${nextTheme}`;
  }

  // 5. Quick Switcher (Ctrl+O)
  openQuickSwitcher() {
    this.quickSwitcherModal.classList.add('open');
    this.qsInput.value = '';
    this.renderQuickSwitcherResults('');
    setTimeout(() => this.qsInput.focus(), 50);
  }

  closeQuickSwitcher() {
    this.quickSwitcherModal.classList.remove('open');
  }

  renderQuickSwitcherResults(filterText) {
    this.qsResults.innerHTML = '';
    const query = filterText.toLowerCase();
    const matches = this.notes.filter((n) => n.title.toLowerCase().includes(query));

    matches.forEach((note, idx) => {
      const li = document.createElement('li');
      li.className = `qs-item ${idx === 0 ? 'selected' : ''}`;
      li.innerHTML = `
        <div style="display:flex; align-items:center; gap:8px;">
          <span>${note.sticker || '📄'}</span>
          <span>${note.title}</span>
        </div>
        <span class="qs-badge">Nota</span>
      `;
      li.addEventListener('click', () => {
        this.openNote(note.id);
        this.closeQuickSwitcher();
      });
      this.qsResults.appendChild(li);
    });
  }

  // 6. Grafo 2D Interativo em Canvas
  initGraph() {
    if (!this.graphCanvas) return;
    this.ctx = this.graphCanvas.getContext('2d');
    this.resizeCanvas();
    window.addEventListener('resize', () => this.resizeCanvas());

    this.graphNodes = [];
    this.graphEdges = [];
    this.updateGraphSimulation();
    this.startGraphAnimation();
  }

  resizeCanvas() {
    if (!this.graphCanvas) return;
    this.graphCanvas.width = this.graphCanvas.parentElement.clientWidth || 800;
    this.graphCanvas.height = this.graphCanvas.parentElement.clientHeight || 600;
  }

  updateGraphSimulation() {
    const width = this.graphCanvas ? this.graphCanvas.width : 800;
    const height = this.graphCanvas ? this.graphCanvas.height : 600;

    this.graphNodes = this.notes.map((note, index) => {
      const angle = (index / Math.max(1, this.notes.length)) * Math.PI * 2;
      const radius = 140;
      return {
        id: note.id,
        title: note.title,
        x: width / 2 + Math.cos(angle) * radius,
        y: height / 2 + Math.sin(angle) * radius,
        vx: (Math.random() - 0.5) * 0.5,
        vy: (Math.random() - 0.5) * 0.5,
        radius: note.id === this.currentNoteId ? 14 : 10,
        color: note.id === this.currentNoteId ? '#8957e5' : '#38bdf8'
      };
    });

    // Conecta em anel ou estrelar
    this.graphEdges = [];
    for (let i = 0; i < this.graphNodes.length; i++) {
      for (let j = i + 1; j < this.graphNodes.length; j++) {
        this.graphEdges.push({ source: this.graphNodes[i], target: this.graphNodes[j] });
      }
    }
  }

  startGraphAnimation() {
    const render = () => {
      if (this.graphPane && this.graphPane.classList.contains('active')) {
        this.drawGraph();
      }
      requestAnimationFrame(render);
    };
    requestAnimationFrame(render);
  }

  drawGraph() {
    const ctx = this.ctx;
    const w = this.graphCanvas.width;
    const h = this.graphCanvas.height;

    ctx.clearRect(0, 0, w, h);

    // Linhas de Conexão
    ctx.strokeStyle = 'rgba(137, 87, 229, 0.25)';
    ctx.lineWidth = 1.5;
    this.graphEdges.forEach((edge) => {
      ctx.beginPath();
      ctx.moveTo(edge.source.x, edge.source.y);
      ctx.lineTo(edge.target.x, edge.target.y);
      ctx.stroke();
    });

    // Nós
    this.graphNodes.forEach((node) => {
      // Glow
      ctx.beginPath();
      ctx.arc(node.x, node.y, node.radius + 4, 0, Math.PI * 2);
      ctx.fillStyle = 'rgba(137, 87, 229, 0.15)';
      ctx.fill();

      // Círculo sólido
      ctx.beginPath();
      ctx.arc(node.x, node.y, node.radius, 0, Math.PI * 2);
      ctx.fillStyle = node.color;
      ctx.fill();

      // Rótulo
      ctx.fillStyle = '#f1f5f9';
      ctx.font = '11px Inter, sans-serif';
      ctx.textAlign = 'center';
      ctx.fillText(node.title, node.x, node.y + node.radius + 14);
    });
  }

  // 7. Configuração de Listeners de Eventos
  bindEvents() {
    // Instalação PWA
    if (this.btnInstall) {
      this.btnInstall.addEventListener('click', async () => {
        if (!this.deferredPrompt) return;
        this.deferredPrompt.prompt();
        const { outcome } = await this.deferredPrompt.userChoice;
        if (outcome === 'accepted') {
          this.btnInstall.style.display = 'none';
        }
        this.deferredPrompt = null;
      });
    }

    // Input de edição
    this.noteTitleInput.addEventListener('input', () => this.saveCurrentNote());
    this.markdownEditor.addEventListener('input', () => this.saveCurrentNote());

    // Botão Nova Nota
    document.getElementById('btn-new-note').addEventListener('click', () => this.createNewNote());

    // Tema
    this.themeBtn.addEventListener('click', () => this.cycleTheme());

    // Abas
    this.tabEditor.addEventListener('click', () => {
      this.tabEditor.classList.add('active');
      this.tabGraph.classList.remove('active');
      this.editorPane.style.display = 'flex';
      this.graphPane.classList.remove('active');
    });

    this.tabGraph.addEventListener('click', () => {
      this.tabGraph.classList.add('active');
      this.tabEditor.classList.remove('active');
      this.editorPane.style.display = 'none';
      this.graphPane.classList.add('active');
      this.resizeCanvas();
    });

    // Atalhos Globais: Ctrl+O para Quick Switcher
    window.addEventListener('keydown', (e) => {
      if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'o') {
        e.preventDefault();
        this.openQuickSwitcher();
      }
      if (e.key === 'Escape' && this.quickSwitcherModal.classList.contains('open')) {
        this.closeQuickSwitcher();
      }
    });

    // Filtro do Quick Switcher
    this.qsInput.addEventListener('input', (e) => {
      this.renderQuickSwitcherResults(e.target.value);
    });

    this.quickSwitcherModal.addEventListener('click', (e) => {
      if (e.target === this.quickSwitcherModal) {
        this.closeQuickSwitcher();
      }
    });

    document.getElementById('btn-quick-switcher').addEventListener('click', () => {
      this.openQuickSwitcher();
    });
  }
}

// Inicializa a aplicação PWA quando o DOM estiver pronto
document.addEventListener('DOMContentLoaded', () => {
  window.gnosisApp = new GnosisApp();
});
