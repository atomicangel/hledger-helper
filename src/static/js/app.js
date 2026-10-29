// hledger Helper - Main Application
// Vanilla JS implementation for maximum compatibility

(function() {
  'use strict';

  // State management
  const state = {
    budgetLocations: [],
    liabilityLocations: [],
    creditLocations: [],
    safeToSpendAccount: '',
    transferJournal: '',
    transferMode: false
  };

  // DOM Elements
  const elements = {
    payeeInput: document.getElementById('payee'),
    splitRows: document.getElementById('split-rows'),
    totalAmount: document.getElementById('total-amount'),
    submitBtn: document.getElementById('submit-transaction'),
    addSplitBtn: document.getElementById('add-split'),
    transferMode: document.getElementById('transfer-mode'),
    syncBtn: document.getElementById('sync-btn'),
    pendingIndicator: document.getElementById('pending-indicator'),
    offlineIndicator: document.getElementById('offline-indicator'),
    statusIndicator: document.getElementById('status-indicator')
  };

  // Initialize application
  async function init() {
    await loadConfig();
    loadSources();
    setupEventListeners();
    // Auto-create first line
    addSplitRow();
    checkNetworkStatus();
    // Check server status every 5 seconds
    setInterval(checkNetworkStatus, 5000);
  }

  // Load configuration
  async function loadConfig() {
    try {
      const response = await fetch('/config.json');
      if (response.ok) {
        const config = await response.json();
        state.budgetLocations = config.budget_locations || [];
        state.liabilityLocations = config.liability_locations || [];
        state.creditLocations = config.credit_locations || [];
        state.safeToSpendAccount = config.safe_to_spend_account || '';
        state.transferJournal = config.transfer_journal || '';
        state.expenseLocations = config.expense_locations || [];  // For expense dropdown grouping
      }
    } catch (e) {
      console.log('No external config found, using defaults');
    }
  }

  // Load payment sources dynamically from API
  // Load source accounts from API
  async function loadSources() {
    try {
      const response = await fetch('/api/categories');
      if (response.ok) {
        const sources = await response.json();
        populateSourceDropdown(sources);
      }
    } catch (e) {
      console.log('Failed to load sources:', e);
    }
  }


  // Helper: group accounts into config-derived sections based on budget_locations/liability_locations prefixes
  // Config prefixes from config.yaml drive the dropdown grouping (e.g., "assets:Cash:Wallet:" → header = "Wallet")
  const groupAccountsIntoSections = (select, items) => {
    select.innerHTML = '<option value="">-- Select --</option>';
    if (!items || !items.length) return;
    
    // Filter out root accounts and accounts without subaccount depth
    items = items.filter(src => src.full_path !== 'liabilities' && 
                    src.full_path !== 'expenses' && 
                    src.display_name.trim() !== '');
    
    const groups = {};
    const configLocs = [...state.budgetLocations, ...state.liabilityLocations, ...state.expenseLocations];
    
    items.forEach(src => {
      let matched = false;
      
      // Match accounts against configured location prefixes from config.yaml
      for (const loc of configLocs) {
        const prefix = loc.replace(/:$/, '');  // Remove trailing colon if present
        // Only match subaccounts that are actually under the prefix (not root level)
        if (src.full_path !== prefix && src.full_path.startsWith(prefix)) {
          // Section header is the last segment after the prefix path
          const parts = prefix.split(':').filter(p => p);
          const headerName = parts[parts.length - 1];
          
          if (headerName) {
            groups[headerName] ||= [];
            groups[headerName].push(src);
          } else {
            // Fallback for malformed prefixes: use account type as section name
            const segments = src.full_path.split(':').filter(p => p);
            const fallbackName = 'Account:' + segments[1] + (segments.slice(2).join(':') || 'Other');
            groups[fallbackName] ||= [];
            groups[fallbackName].push(src);
          }
          matched = true; break;
        }
      }
      
      // Fallback for any unmatched accounts using account segments  
      if (!matched && src.full_path) {
        const segments = src.full_path.split(':').filter(p => p);
        if (segments.length >= 3) {
          const fallbackName = 'Account:' + segments[1] + (segments.slice(2).join(':') || 'Other');
          groups[fallbackName] ||= [];
          groups[fallbackName].push(src);
        }
      }
    });
    
    // Render optgroups sorted alphabetically by header name (will be bold via CSS .optgroup[aria-labelledby])
    Object.keys(groups).sort().forEach(header => {
      const items = groups[header];
      if (!items || !items.length) return;
      
      items.sort((a, b) => a.display_name.localeCompare(b.display_name));
      
      const optgroup = document.createElement('optgroup');
      optgroup.label = header.charAt(0).toUpperCase() + header.slice(1);  // Capitalize display header (e.g., "expenses" -> "Expenses")
      
      items.forEach(src => {
        const option = document.createElement('option');
        option.value = src.full_path;
        option.textContent = src.display_name || src.full_path.split(':').pop();
        optgroup.appendChild(option);
      });
      
      select.appendChild(optgroup);
    });
  };


  // Populate source dropdown with config-derived grouping using shared helper function  
  function populateSourceDropdown(sources) {
    const select = document.getElementById('source-account');
    if (!select) return;
    
    // Ensure option element exists before calling helper
    select.innerHTML = '<option value="">-- Select Source --</option>';
    
    // Pass sources to helper for grouping by config-derived section headers
    groupAccountsIntoSections(select, sources);
  }


  async function loadBudgets() {
    try {
      // Load safe-to-spend hero data if available
      let safeToSpendData = null;
      try {
        const s2sResp = await fetch('/api/safe-to-spend', { cache: 'no-store' });
        if (s2sResp.ok) {
          const s2sData = await s2sResp.json();
          if (s2sData.account && s2sData.balance !== null) safeToSpendData = s2sData;
        }
      } catch(e) { console.warn('Safe-to-spend endpoint unavailable'); }

      // Use existing /api/expenses-totals endpoint instead of deleted /api/budgets
      const response = await fetch('/api/expenses-totals', { cache: 'no-store' });
      
      if (!response.ok && !safeToSpendData) {
        console.log('Budget tab not available yet, skipping...');
        return;
      }

      // Get grouped data from API - already sorted by header in backend
      const apiData = await response.json();
      const totals = Array.isArray(apiData) ? apiData : (apiData.entries || []);
      const headerBalances = (!Array.isArray(apiData) && apiData.header_balances) ? apiData.header_balances : {};
      renderDynamicBudgetTab(safeToSpendData, totals, headerBalances);
    } catch (e) {
      console.error('Failed to load budgets:', e);
    }
  }

  function renderDynamicBudgetTab(safeToSpendData, totals, headerBalances) {
    // Safe-to-Spend hero: rendered into the .safe-to-spend-hero container at the
    // top of the Budget tab (see index.html). Simple-style: label + one big number.
    const heroEl = document.querySelector('#budget .safe-to-spend-hero');
    if (heroEl) {
      if (safeToSpendData && safeToSpendData.account) {
        const bal = safeToSpendData.balance;
        const color = bal > 0 ? 'var(--success)' : 'var(--danger)';
        const sign = bal < 0 ? '-' : '';
        heroEl.classList.remove('hidden');
        heroEl.innerHTML =
          '<div class="hero-label">SAFE TO SPEND</div>' +
          '<div class="hero-amount" style="color:' + color + '">' +
          sign + '$' + Math.abs(bal).toFixed(2) + '</div>' +
          (safeToSpendData.overspent < 0
            ? '<div class="hero-breakdown">$' + safeToSpendData.unallocated.toFixed(2) +
              ' unallocated - $' + Math.abs(safeToSpendData.overspent).toFixed(2) + ' overspent</div>'
            : '');
      } else {
        heroEl.classList.add('hidden');
        heroEl.innerHTML = '';
      }
    }

    // Group budget items by header field for dynamic rendering
    const groups = {};
    
    totals.forEach(item => {
      // Use header field from API response as group key (Sorting already done in backend)
      const header = item.header || 'Unknown';
      if (!groups[header]) {
        groups[header] = [];
      }
      groups[header].push(item);
    });
    
    // Create dynamic container if needed
    
    const containerEl = document.getElementById('budget-groups-container');
    if (!containerEl) return;
    // Clear existing cards before re-rendering (fixes refresh duplicating content)
    containerEl.innerHTML = '';

    // Render each group's budget items dynamically
    for (const header in groups) {
      const items = groups[header];
      if (!items || !items.length) continue;
      
      const container = document.createElement('div');
      container.id = 'group-' + header.replace(/[^a-zA-Z0-9]/g, '-');
      container.className = 'budget-group';
      containerEl.appendChild(container);
      
      // Create header label for this group
      const headerDiv = document.createElement('div');
      headerDiv.className = 'budget-group-label';
      headerDiv.textContent = header;
      if (typeof headerBalances !== "undefined" && headerBalances && headerBalances[header] !== undefined) {
        const balSpan = document.createElement("span");
        balSpan.className = "header-balance";
        const hbal = headerBalances[header];
        balSpan.textContent = " " + (hbal < 0 ? "-" : "") + "\$" + Math.abs(hbal).toFixed(2);
        if (hbal < 0) balSpan.style.color = "#cc0000";
        headerDiv.appendChild(balSpan);
      }
      containerEl.insertBefore(headerDiv, container);  // Insert header before items
      
      // Append budget cards for this group's items (already sorted by backend)
      items.forEach(item => {
        const card = createBudgetCard(item);
        container.appendChild(card);
      });
    }
  }

  function createBudgetCard(item) {
    const card = document.createElement('div');
    card.className = 'budget-card';

    // Title: display_name from API response (user-friendly label)
    const title = document.createElement('div');
    title.className = 'budget-title';
    title.textContent = item.display_name;
    card.appendChild(title);

    // Progress bar: FUNDED fraction (allocated/budget). Underwater envelopes get an empty bar.
    const isLiability = item.kind === 'liabilities';
    const underwater = !isLiability && item.balance < 0;
    const over = !underwater && !isLiability && item.remaining < 0;
    const under = !underwater && !isLiability && item.spent_total < item.budget_limit;
    const funded = item.budget_limit > 0
      ? Math.max(0, Math.min(1, item.spent_total / item.budget_limit))
      : 0;
    const barWrap = document.createElement('div');
    barWrap.className = 'bar-wrap';
    const barFill = document.createElement('div');
    barFill.className = 'bar-fill' + (under ? ' partial' : '');
    const widthFrac = underwater ? 0 : (isLiability ? funded : (!under ? 1 : funded));
    barFill.style.width = (widthFrac * 100).toFixed(1) + '%';
    barWrap.appendChild(barFill);
    card.appendChild(barWrap);

    // Meta: underwater = bold red "$X overspent"; otherwise funded amounts "$X of $Y funded"
    const meta = document.createElement('div');
    meta.className = 'budget-meta';
    const fmt = n => '$' + Math.abs(n).toFixed(2);
    if (isLiability) {
      meta.textContent = `${fmt(item.spent_total)} owed`;
      meta.style.color = 'var(--success)';
    } else if (underwater) {
      meta.textContent = `${fmt(item.balance)} overspent`;
      meta.style.color = 'var(--danger)';
      meta.style.fontWeight = 'bold';
    } else if (over) {
      meta.textContent = `${fmt(item.remaining)} over budget`;
      meta.style.color = 'var(--success)';
    } else {
      meta.textContent = `${fmt(item.spent_total)} of ${fmt(item.budget_limit)} funded`;
      if (under) {
        meta.style.color = '#ffdd00';
        meta.style.fontWeight = 'bold';
      } else {
        meta.style.color = 'var(--success)';
      }
    }
    card.appendChild(meta);

    return card;
  }



  // Setup event listeners
  function setupEventListeners() {
    // Tab switching
    document.querySelectorAll('.tab-btn').forEach(btn => {
      btn.addEventListener('click', (e) => switchTab(e.target.dataset.tab));
    });

    // Budget refresh button: re-fetch data without full page reload
    const refreshBtn = document.getElementById('budget-refresh');
    if (refreshBtn) {
      refreshBtn.addEventListener('click', async () => {
        refreshBtn.classList.add('spinning');
        refreshBtn.disabled = true;
        try {
          await loadBudgets();
        } finally {
          refreshBtn.classList.remove('spinning');
          refreshBtn.disabled = false;
        }
      });
    }

    // Transfer mode toggle
    if (elements.transferMode) {
      elements.transferMode.addEventListener('change', (e) => {
        state.transferMode = e.target.checked;
        // Rebuild split rows to reflect mode change
        elements.splitRows.innerHTML = '';
        addSplitRow();
        updateTotal();
      });
    }

    // Add split button
    elements.addSplitBtn.addEventListener('click', addSplitRow);

    // Submit transaction
    elements.submitBtn.addEventListener('click', submitTransaction);

    // Sync button
    elements.syncBtn.addEventListener('click', syncOfflineTransactions);

    // Network status
    window.addEventListener('online', () => checkNetworkStatus());
    window.addEventListener('offline', () => updateNetworkStatus(false));
  }

  // Switch tabs
  function switchTab(tab) {
    document.querySelectorAll('.tab-btn').forEach(btn => {
      btn.classList.toggle('active', btn.dataset.tab === tab);
    });
    document.querySelectorAll('.tab-content').forEach(content => {
      content.classList.toggle('active', content.id === tab);
    });
    if (tab === 'budget') {
      loadBudgets();
    }
  }

  // Network status
  function checkNetworkStatus() {
    // Check if server is actually responding
    fetch('/config.json', { method: 'HEAD', cache: 'no-store' })
      .then(() => updateNetworkStatus(true))
      .catch(() => updateNetworkStatus(false));
  }

  function updateNetworkStatus(isOnline) {
    elements.statusIndicator.textContent = isOnline ? 'Online' : 'Offline';
    elements.statusIndicator.style.color = isOnline ? 'white' : '#ff9800';
    
    getPendingCount().then(pending => {
      if (pending > 0) {
        elements.pendingIndicator.textContent = `${pending} pending`;
        elements.pendingIndicator.classList.add('has-pending');
        elements.syncBtn.classList.remove('hidden');
      } else {
        elements.pendingIndicator.textContent = '0 pending';
        elements.pendingIndicator.classList.remove('has-pending');
        elements.syncBtn.classList.add('hidden');
      }
    });
  }

  // Get pending transactions count from IndexedDB
  async function getPendingCount() {
    return new Promise((resolve) => {
      const request = indexedDB.open('hledgerHelper', 1);
      request.onupgradeneeded = (e) => {
        const db = e.target.result;
        if (!db.objectStoreNames.contains('queue')) {
          db.createObjectStore('queue', { keyPath: 'id', autoIncrement: true });
        }
      };
      request.onsuccess = (e) => {
        const db = e.target.result;
        const transaction = db.transaction('queue', 'readonly');
        const store = transaction.objectStore('queue');
        const countRequest = store.count();
        countRequest.onsuccess = () => resolve(countRequest.result);
        countRequest.onerror = () => resolve(0);
      };
      request.onerror = () => resolve(0);
    });
  }

  // IndexedDB operations
  async function addToQueue(transactionData) {
    return new Promise((resolve, reject) => {
      const request = indexedDB.open('hledgerHelper', 1);
      request.onupgradeneeded = (e) => {
        const db = e.target.result;
        if (!db.objectStoreNames.contains('queue')) {
          db.createObjectStore('queue', { keyPath: 'id', autoIncrement: true });
        }
      };
      request.onsuccess = (e) => {
        const db = e.target.result;
        const idbTransaction = db.transaction('queue', 'readwrite');
        const store = idbTransaction.objectStore('queue');
        // Queue item: { transaction, covers, timestamp, synced }. transaction
        // is null when the purchase already landed and only the cover is
        // queued; covers is [] for plain non-card submits. A legacy string
        // argument, or an older item without covers, means covers: [].
        const item = (transactionData && typeof transactionData === 'object')
          ? { transaction: transactionData.transaction ?? null, covers: transactionData.covers || [] }
          : { transaction: transactionData, covers: [] };
        store.add({ transaction: item.transaction, covers: item.covers, timestamp: Date.now(), synced: false });
        idbTransaction.oncomplete = () => resolve();
        idbTransaction.onerror = (e) => reject(e);
      };
      request.onerror = (e) => reject(e);
    });
  }

  // --- Credit-card auto-cover ---
  // A split row is a "credit" row when its source is a subaccount of any
  // credit_locations prefix (config-driven; never a hardcoded prefix).
  function isCreditSource(source) {
    if (!source) return false;
    return (state.creditLocations || []).some(loc => {
      const prefix = loc.replace(/:$/, '');
      return source !== prefix && source.startsWith(prefix);
    });
  }

  // Envelope candidates for cover-from / cover-to: expense-typed categories
  // that are not liabilities and not the bare Budget container.
  function coverEnvelopeOptions(categories) {
    if (!categories) return [];
    return categories.filter(c =>
      c.category_type === 'expense' &&
      !c.full_path.startsWith('liabilities:') &&
      c.full_path !== 'assets:Bank:Checking:Budget' &&
      c.display_name && c.display_name.trim() !== '');
  }

  // Suffix of an expense path for envelope matching: expenses:Food:Groceries -> Food:Groceries
  function expenseSuffix(dest) {
    if (!dest) return '';
    return dest.startsWith('expenses:') ? dest.slice('expenses:'.length) : dest;
  }

  // Default cover-from for a row: the unique envelope whose display name
  // matches the row's expense suffix (case-insensitive). Zero matches or
  // multiple matches (duplicate Checking/Savings envelopes) -> Unallocated
  // fallback (safe_to_spend_account first, then a display-name match).
  // Misc is NOT special-cased: it lands on Unallocated simply because no
  // envelope matches it.
  function defaultCoverFrom(dest, options) {
    const suffix = expenseSuffix(dest).toLowerCase();
    const matches = options.filter(o => o.display_name.toLowerCase() === suffix);
    if (matches.length === 1) return matches[0].full_path;
    if (state.safeToSpendAccount && options.some(o => o.full_path === state.safeToSpendAccount)) {
      return state.safeToSpendAccount;
    }
    const unalloc = options.find(o => o.display_name.toLowerCase() === 'unallocated');
    return unalloc ? unalloc.full_path : '';
  }

  // Card envelope for a credit source: the envelope whose last path segment
  // matches the source's last segment (Apple -> Budget:Apple). Unique match
  // only; otherwise null (caller shows the fallback dropdown).
  function cardEnvelopeFor(source, options) {
    const last = source.split(':').filter(Boolean).pop();
    if (!last) return null;
    const matches = options.filter(o => {
      const seg = o.full_path.split(':').filter(Boolean).pop();
      return seg && seg.toLowerCase() === last.toLowerCase();
    });
    return matches.length === 1 ? matches[0] : null;
  }

  // Show/hide and (re)compute a row's cover UI. Called on source change,
  // expense change, and once the row's category list arrives.
  function refreshCoverRow(row) {
    const coverWrap = row.querySelector('.cover-row');
    if (!coverWrap) return;
    const source = row.querySelector('.split-source').value.trim();
    if (!isCreditSource(source)) {
      coverWrap.style.display = 'none';
      return;
    }
    const options = row._coverOptions || [];
    const coverFromSelect = row.querySelector('.cover-from');
    if (!coverFromSelect.dataset.populated && options.length > 0) {
      groupAccountsIntoSections(coverFromSelect, options);
      coverFromSelect.dataset.populated = '1';
    }
    // Default cover-from, unless the user already overrode it on this row.
    if (!row.dataset.coverFromTouched) {
      const dest = row.querySelector('.split-expense').value.trim();
      const def = defaultCoverFrom(dest, options);
      if (def) coverFromSelect.value = def;
    }
    // Cover-to: matched card envelope as text, else a fallback dropdown.
    const cardEnv = cardEnvelopeFor(source, options);
    const coverToText = row.querySelector('.cover-to-text');
    const coverToSelect = row.querySelector('.cover-to');
    if (cardEnv) {
      coverToText.textContent = cardEnv.display_name;
      coverToText.style.display = '';
      coverToSelect.style.display = 'none';
      coverWrap.dataset.coverTo = cardEnv.full_path;
    } else {
      coverToText.style.display = 'none';
      if (!coverToSelect.dataset.populated && options.length > 0) {
        groupAccountsIntoSections(coverToSelect, options);
        coverToSelect.dataset.populated = '1';
      }
      coverToSelect.style.display = '';
      coverWrap.dataset.coverTo = coverToSelect.value;
    }
    coverWrap.style.display = 'flex';
  }

  // Build one cover entry per (card, card-envelope) group, with postings
  // summed per cover-from envelope: Food -$60, Clothing -$15, card +$75.
  // The cover payee is the purchase payee verbatim, so the pair is findable
  // by name across the two journals.
  function buildCoverStrings(cardRows, date, payee) {
    const groups = new Map();
    cardRows.forEach(r => {
      const key = r.source + '\0' + r.coverTo;
      if (!groups.has(key)) groups.set(key, { coverTo: r.coverTo, byFrom: new Map() });
      const g = groups.get(key);
      g.byFrom.set(r.coverFrom, (g.byFrom.get(r.coverFrom) || 0) + r.amount);
    });
    const covers = [];
    groups.forEach(g => {
      let lines = `${date} ! ${payee}`;
      let total = 0;
      g.byFrom.forEach((amt, from) => {
        lines += `\n  ${from}  -$${amt.toFixed(2)}`;
        total += amt;
      });
      lines += `\n  ${g.coverTo}  $${total.toFixed(2)}`;
      covers.push(lines + '\n');
    });
    return covers;
  }

  // Update a queued item in place (used to drop landed parts so retries
  // never double-post). Resolves even on error; leftovers stay queued.
  function updateQueuedItem(data) {
    return new Promise((resolve) => {
      const request = indexedDB.open('hledgerHelper', 1);
      request.onsuccess = (e) => {
        const db = e.target.result;
        const tx = db.transaction('queue', 'readwrite');
        const putReq = tx.objectStore('queue').put(data);
        putReq.onsuccess = () => resolve();
        putReq.onerror = () => resolve();
      };
      request.onerror = () => resolve();
    });
  }

  // Split row management
  function addSplitRow() {
    const row = document.createElement('div');
    row.className = 'split-row';
    
    // Source select
    const sourceSelect = document.createElement('select');
    sourceSelect.className = 'split-source';
    sourceSelect.innerHTML = '<option value="">-- Source --</option>';
    
    // Expense/Destination select
    const expenseSelect = document.createElement('select');
    expenseSelect.className = 'split-expense';
    expenseSelect.innerHTML = state.transferMode ? '<option value="">-- Destination --</option>' : '<option value="">-- Expense --</option>';
    
    // Amount input
    const amountInput = document.createElement('input');
    amountInput.type = 'number';
    amountInput.className = 'split-amount';
    amountInput.step = '0.01';
    amountInput.placeholder = '0.00';
    
    row.appendChild(sourceSelect);
    row.appendChild(expenseSelect);
    row.appendChild(amountInput);
    elements.splitRows.appendChild(row);
    
    // Add listeners
    amountInput.addEventListener('input', updateTotal);
    amountInput.addEventListener('change', updateTotal);
    sourceSelect.addEventListener('change', () => {
      // Fresh card, fresh default: a manual cover-from pick for the old
      // source must not carry over.
      delete row.dataset.coverFromTouched;
      refreshCoverRow(row);
    });
    expenseSelect.addEventListener('change', () => refreshCoverRow(row));
    
    // Populate source and expense using shared helper function
    fetch('/api/categories')
      .then(res => res.json())
      .then(sources => {
        groupAccountsIntoSections(sourceSelect, sources);
        // Cover options come from the same category list.
        const cw = row.querySelector('.cover-row');
        if (cw) {
          row._coverOptions = coverEnvelopeOptions(sources);
          refreshCoverRow(row);
        }
      });
    if (state.transferMode) {
      // Populate destination select with categories (sources)
      fetch('/api/categories')
        .then(res => res.json())
        .then(sources => {
          groupAccountsIntoSections(expenseSelect, sources);
        });
    } else {
      fetch('/api/expenses')
        .then(res => res.json())
        .then(expenses => {
          const filtered = expenses.filter(e => e.full_path !== 'liabilities' && 
                    e.full_path !== 'expenses' && 
                    !e.full_path.includes(':Budget:') && 
                    e.display_name.trim() !== '');
          groupAccountsIntoSections(expenseSelect, filtered);
        });
    }
    
    // Add remove button
    const removeBtn = document.createElement('button');
    removeBtn.textContent = '×';
    removeBtn.className = 'btn-secondary';
    removeBtn.style.padding = '8px 12px';
    removeBtn.style.borderRadius = '4px';
    removeBtn.style.border = 'none';
    removeBtn.style.background = 'var(--error-color)';
    removeBtn.style.color = 'white';
    removeBtn.style.cursor = 'pointer';
    removeBtn.style.marginLeft = '8px';
    removeBtn.onclick = () => {
      row.remove();
      updateTotal();
    };
    row.appendChild(removeBtn);

    // Credit-card auto-cover UI (normal mode only; transfer-mode rows are
    // rebuilt on toggle, so they never get one). Full-width sub-row.
    if (!state.transferMode) {
      const coverWrap = document.createElement('div');
      coverWrap.className = 'cover-row';
      coverWrap.style.display = 'none';
      coverWrap.style.gridColumn = '1 / -1';
      coverWrap.style.alignItems = 'center';
      coverWrap.style.gap = '8px';
      const coverLabel = document.createElement('span');
      coverLabel.textContent = 'Cover from:';
      const coverFromSelect = document.createElement('select');
      coverFromSelect.className = 'cover-from';
      const coverToText = document.createElement('span');
      coverToText.className = 'cover-to-text';
      const coverToSelect = document.createElement('select');
      coverToSelect.className = 'cover-to';
      coverToSelect.style.display = 'none';
      coverFromSelect.addEventListener('change', () => { row.dataset.coverFromTouched = '1'; });
      coverToSelect.addEventListener('change', () => { coverWrap.dataset.coverTo = coverToSelect.value; });
      coverWrap.appendChild(coverLabel);
      coverWrap.appendChild(coverFromSelect);
      coverWrap.appendChild(coverToText);
      coverWrap.appendChild(coverToSelect);
      row.appendChild(coverWrap);
    }
    
    updateTotal();
  }

  // Calculate total (for split transactions)
  function updateTotal() {
    let total = 0;
    
    document.querySelectorAll('.split-amount').forEach(input => {
      total += parseFloat(input.value) || 0;
    });
    
    elements.totalAmount.textContent = '$' + total.toFixed(2);
  }




  // Submit transaction
  async function submitTransaction() {
    const payee = elements.payeeInput.value.trim();
    
    if (!payee) {
      alert('Please enter a valid payee');
      return;
    }

    const rows = Array.from(document.querySelectorAll('.split-row')).map(row => {
      const coverWrap = row.querySelector('.cover-row');
      const coverShown = coverWrap && coverWrap.style.display !== 'none';
      return {
        source: row.querySelector('.split-source').value.trim(),
        dest: row.querySelector('.split-expense').value.trim(),
        amount: parseFloat(row.querySelector('.split-amount').value) || 0,
        coverFrom: coverShown ? row.querySelector('.cover-from').value.trim() : '',
        coverTo: coverShown ? (coverWrap.dataset.coverTo || '').trim() : ''
      };
    });

    const validRows = rows.filter(r => r.source && r.dest && r.amount > 0);
    if (validRows.length === 0) {
      alert('Please add at least one line with source, destination and amount');
      return;
    }

    if (state.transferMode) {
      // Submit as transfers
      try {
        const promises = validRows.map(r => fetch('/api/transfers', {
          method: 'POST',
          headers: { 'Content-Type': 'application/json' },
          body: JSON.stringify({ source_account: r.source, dest_account: r.dest, amount: r.amount, payee })
        }));
        const responses = await Promise.all(promises);
        const ok = responses.every(res => res.ok);
        if (ok) {
          elements.payeeInput.value = '';
          elements.splitRows.innerHTML = '';
          addSplitRow();
          elements.totalAmount.textContent = '$0.00';
          alert('Transfer(s) submitted successfully!');
        } else {
          throw new Error('One or more transfers failed');
        }
      } catch (e) {
        console.error('Transfer failed:', e);
        alert('Transfer failed');
      }
      return;
    }

    // Build transaction string in hledger format
    const now = new Date();
    const date = now.getFullYear() + '-' + String(now.getMonth()+1).padStart(2,'0') + '-' + String(now.getDate()).padStart(2,'0');
    // Credit-card auto-cover: validate cover data BEFORE posting anything,
    // so a missing cover envelope can never produce a half-submit.
    const cardRows = validRows.filter(r => isCreditSource(r.source));
    const incompleteCover = cardRows.filter(r => !r.coverFrom || !r.coverTo);
    if (incompleteCover.length > 0) {
      alert('Please choose a cover envelope for each card line (Cover from and card envelope)');
      return;
    }
    const coverStrs = buildCoverStrings(cardRows, date, payee);

    let transactionLines = `${date} ! ${payee}`;
    validRows.forEach(r => {
      transactionLines += `\n  ${r.dest}  $${r.amount.toFixed(2)}`;
      transactionLines += `\n  ${r.source}  -$${r.amount.toFixed(2)}`;
    });
    const transactionStr = transactionLines + '\n';

    // Try to submit
    try {
      const response = await fetch('/api/transactions', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ transaction: transactionStr })
      });

      if (!response.ok) {
        const error = await response.text();
        throw new Error(error);
      }
      // The backend answers validation failures as HTTP 200 with
      // {valid:false} (hledger check runs before the write, and a missing
      // target file fails it). HTTP ok alone is NOT success: a
      // valid:false purchase must throw so it queues instead of
      // reporting a false success and losing the transaction.
      const purchaseData = await response.json().catch(() => ({}));
      if (purchaseData.valid === false) {
        throw new Error(purchaseData.message || purchaseData.error || 'Transaction validation failed');
      }
      // Purchase landed; now post the cover entries (one per card/envelope
      // group) to the configured transfer journal via target_file. /api/transfers only
      // writes two-posting transfers, so a summed multi-posting cover goes
      // through /api/transactions instead -- same file, one entry, no new
      // endpoint.
      if (coverStrs.length > 0) {
        const coverTarget = state.transferJournal;
        let failedAt = -1;
        let coverError = '';
        for (let i = 0; i < coverStrs.length; i++) {
          const cres = await fetch('/api/transactions', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ transaction: coverStrs[i], target_file: coverTarget })
          });
          if (!cres.ok) { coverError = await cres.text(); failedAt = i; break; }
          const cdata = await cres.json().catch(() => ({}));
          if (cdata.valid === false) { coverError = cdata.message || cdata.error || 'Cover validation failed'; failedAt = i; break; }
        }
        if (failedAt >= 0) {
          // Purchase is already in the journal; the remaining cover is not.
          // Never a silent half-submit: queue only the unlanded covers and
          // show their full text for manual entry. Form is cleared so the
          // purchase cannot be double-posted by resubmitting.
          const remaining = coverStrs.slice(failedAt);
          await addToQueue({ transaction: null, covers: remaining });
          updateNetworkStatus(navigator.onLine);
          elements.payeeInput.value = '';
          elements.splitRows.innerHTML = '';
          addSplitRow();
          elements.totalAmount.textContent = '$0.00';
          alert('Purchase saved, but the cover transfer FAILED and was queued for retry. Cover text for manual entry:\n\n' + remaining.join('\n') + '\nError: ' + coverError);
          return;
        }
      }
      // Success - clear form
      elements.payeeInput.value = '';
      elements.splitRows.innerHTML = '';
      addSplitRow();
      elements.totalAmount.textContent = '$0.00';
      alert('Transaction submitted successfully!');
    } catch (e) {
      console.error('Transaction failed:', e);
      await addToQueue({ transaction: transactionStr, covers: coverStrs });
      updateNetworkStatus(navigator.onLine);
      alert('Transaction queued for offline sync');
    }
  }

  // Sync offline transactions
  async function syncOfflineTransactions() {
    const request = indexedDB.open('hledgerHelper', 1);
    request.onsuccess = (e) => {
      const db = e.target.result;
      const transaction = db.transaction('queue', 'readonly');
      const store = transaction.objectStore('queue');
      const getAll = store.getAll();
      
      getAll.onsuccess = async () => {
        const items = getAll.result;
        if (items.length === 0) {
          if (elements.offlineIndicator) {
            elements.offlineIndicator.classList.add('hidden');
          }
          return;
        }

        // Try to sync each item
        for (const item of items) {
          try {
            // Sync purchase first, then its cover entries. Each landed part
            // is dropped from the queued item immediately, so a later retry
            // can never double-post it. The item is deleted only when the
            // purchase and every cover have landed.
            let ok = true;
            if (item.transaction) {
              const response = await fetch('/api/transactions', {
                method: 'POST',
                headers: { 'Content-Type': 'application/json' },
                body: JSON.stringify({ transaction: item.transaction })
              });
              if (!response.ok) {
                ok = false;
              } else {
                const rdata = await response.json().catch(() => ({}));
                if (rdata.valid === false) {
                  ok = false;
                } else {
                  await updateQueuedItem({ ...item, transaction: null });
                }
              }
            }
            if (ok) {
              const covers = item.covers || [];
              const coverTarget = state.transferJournal;
              for (let i = 0; i < covers.length; i++) {
                const cres = await fetch('/api/transactions', {
                  method: 'POST',
                  headers: { 'Content-Type': 'application/json' },
                  body: JSON.stringify({ transaction: covers[i], target_file: coverTarget })
                });
                if (!cres.ok) { ok = false; break; }
                const cdata = await cres.json().catch(() => ({}));
                if (cdata.valid === false) { ok = false; break; }
                await updateQueuedItem({ ...item, transaction: null, covers: covers.slice(i + 1) });
              }
            }
            if (ok) {
              // Delete item after successful sync - open new transaction
              const deleteRequest = indexedDB.open('hledgerHelper', 1);
              deleteRequest.onsuccess = (e) => {
                const db2 = e.target.result;
                const tx = db2.transaction('queue', 'readwrite');
                tx.objectStore('queue').delete(item.id);
              };
            }
          } catch (e) {
            console.error('Failed to sync transaction:', e);
          }
        }

        // Update UI after all sync attempts
        setTimeout(() => {
          getPendingCount().then(pending => {
            if (elements.offlineIndicator) {
              if (pending === 0) {
                elements.offlineIndicator.classList.add('hidden');
              }
            }
            if (elements.pendingIndicator) {
              if (pending === 0) {
                elements.pendingIndicator.textContent = '0 pending';
                elements.pendingIndicator.classList.remove('has-pending');
                elements.syncBtn.classList.add('hidden');
              } else {
                elements.pendingIndicator.textContent = `${pending} pending`;
                elements.pendingIndicator.classList.add('has-pending');
                elements.syncBtn.classList.remove('hidden');
              }
            }
          });
        }, 500);
      };
    };
  }

  // Initialize on DOM ready
  document.addEventListener('DOMContentLoaded', init);

  // Handle service worker registration
  if ('serviceWorker' in navigator) {
    window.addEventListener('load', () => {
      navigator.serviceWorker.register('/sw.js')
        .then(reg => console.log('SW registered:', reg))
        .catch(err => console.log('SW registration failed:', err));
    });
  }
})();
