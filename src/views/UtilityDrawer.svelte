<!--
  UtilityDrawer.svelte: Minimalist centralized utility drawer
  Micro-apps for quick tasks without loading heavy software.
  Integrates with Windows native tools (Calculator, Notepad, System settings).
-->
<script>
  import { call } from '../lib/api.js';
  import { toast } from '../lib/state.svelte.js';
  import Icon from '../lib/Icon.svelte';

  // Quick calculator state
  let calcDisplay = $state('0');
  let calcPrev = $state(null);
  let calcOp = $state(null);
  let calcNew = $state(true);

  // Quick scratchpad state
  let scratchpad = $state('');

  async function loadScratchpad() {
    try {
      scratchpad = (await call('setting_get', { key: 'scratchpad' })) ?? '';
    } catch (e) {
      scratchpad = '';
    }
  }
  loadScratchpad();

  async function saveScratchpad() {
    await call('setting_set', { key: 'scratchpad', value: scratchpad });
  }

  // Calculator functions
  function calcInput(n) {
    if (calcNew) {
      calcDisplay = n;
      calcNew = false;
    } else {
      calcDisplay = calcDisplay === '0' ? n : calcDisplay + n;
    }
  }

  function calcOpSet(op) {
    if (calcPrev !== null && !calcNew) {
      calcCalculate();
    }
    calcPrev = parseFloat(calcDisplay);
    calcOp = op;
    calcNew = true;
  }

  function calcCalculate() {
    if (calcPrev === null || calcOp === null) return;
    const current = parseFloat(calcDisplay);
    let result;
    switch (calcOp) {
      case '+': result = calcPrev + current; break;
      case '-': result = calcPrev - current; break;
      case '×': result = calcPrev * current; break;
      case '÷': result = calcPrev / current; break;
      default: return;
    }
    calcDisplay = String(result);
    calcPrev = null;
    calcOp = null;
    calcNew = true;
  }

  function calcClear() {
    calcDisplay = '0';
    calcPrev = null;
    calcOp = null;
    calcNew = true;
  }

  // Windows native tools
  function openCalc() {
    try {
      call('open_native', { app: 'calc' });
      toast('Calculator opened');
    } catch (e) {
      toast('Failed to open Calculator');
    }
  }

  function openNotepad() {
    try {
      call('open_native', { app: 'notepad' });
      toast('Notepad opened');
    } catch (e) {
      toast('Failed to open Notepad');
    }
  }

  function openSettings() {
    try {
      call('open_native', { app: 'settings' });
      toast('Settings opened');
    } catch (e) {
      toast('Failed to open Settings');
    }
  }
</script>

<div class="utility-drawer">
  <div class="section">
    <h3>Quick Calculator</h3>
    <div class="calculator">
      <div class="calc-display">{calcDisplay}</div>
      <div class="calc-buttons">
        <button class="calc-btn op" onclick={calcClear}>C</button>
        <button class="calc-btn op" onclick={() => calcOpSet('÷')}>÷</button>
        <button class="calc-btn op" onclick={() => calcOpSet('×')}>×</button>
        <button class="calc-btn op" onclick={() => calcOpSet('-')}>−</button>

        <button class="calc-btn num" onclick={() => calcInput('7')}>7</button>
        <button class="calc-btn num" onclick={() => calcInput('8')}>8</button>
        <button class="calc-btn num" onclick={() => calcInput('9')}>9</button>
        <button class="calc-btn op" onclick={() => calcOpSet('+')}>+</button>

        <button class="calc-btn num" onclick={() => calcInput('4')}>4</button>
        <button class="calc-btn num" onclick={() => calcInput('5')}>5</button>
        <button class="calc-btn num" onclick={() => calcInput('6')}>6</button>
        <button class="calc-btn eq" onclick={calcCalculate} style="grid-row: span 2">=</button>

        <button class="calc-btn num" onclick={() => calcInput('1')}>1</button>
        <button class="calc-btn num" onclick={() => calcInput('2')}>2</button>
        <button class="calc-btn num" onclick={() => calcInput('3')}>3</button>

        <button class="calc-btn num" onclick={() => calcInput('0')} style="grid-column: span 2">0</button>
        <button class="calc-btn num" onclick={() => calcInput('.')}>.</button>
      </div>
    </div>
  </div>

  <div class="section">
    <h3>Quick Scratchpad</h3>
    <textarea
      class="scratchpad"
      bind:value={scratchpad}
      onblur={saveScratchpad}
      placeholder="Type quick notes here..."
      aria-label="Quick scratchpad"
    ></textarea>
  </div>

  <div class="section">
    <h3>Windows Tools</h3>
    <div class="tools-grid">
      <button class="tool-card" onclick={openCalc}>
        <Icon name="system" size={24} />
        <span>Calculator</span>
      </button>
      <button class="tool-card" onclick={openNotepad}>
        <Icon name="file" size={24} />
        <span>Notepad</span>
      </button>
      <button class="tool-card" onclick={openSettings}>
        <Icon name="settings" size={24} />
        <span>Settings</span>
      </button>
    </div>
  </div>
</div>

<style>
  .utility-drawer {
    padding: 16px;
    height: 100%;
    overflow-y: auto;
  }

  .section {
    margin-bottom: 24px;
  }

  .section h3 {
    margin: 0 0 12px 0;
    font-size: 14px;
    font-weight: 600;
    color: var(--dim);
    text-transform: uppercase;
    letter-spacing: 0.5px;
  }

  /* Calculator */
  .calculator {
    background: var(--row);
    border-radius: 12px;
    padding: 12px;
  }

  .calc-display {
    background: #0a0a0a;
    border-radius: 8px;
    padding: 12px;
    text-align: right;
    font-size: 24px;
    font-weight: 500;
    margin-bottom: 12px;
    min-height: 48px;
    display: flex;
    align-items: center;
    justify-content: flex-end;
  }

  .calc-buttons {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
  }

  .calc-btn {
    background: var(--row-hi);
    border: none;
    border-radius: 8px;
    padding: 12px;
    font-size: 18px;
    font-weight: 500;
    cursor: pointer;
    transition: background 0.15s;
    color: var(--ink);
  }

  .calc-btn:hover {
    background: var(--accent);
  }

  .calc-btn.op {
    color: var(--accent);
  }

  .calc-btn.eq {
    background: var(--accent);
    color: #0a0a0a;
  }

  .calc-btn.eq:hover {
    filter: brightness(1.1);
  }

  /* Scratchpad */
  .scratchpad {
    width: 100%;
    height: 120px;
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 12px;
    padding: 12px;
    font-size: 14px;
    color: var(--ink);
    resize: none;
    font-family: inherit;
  }

  .scratchpad:focus {
    outline: 2px solid var(--accent);
    outline-offset: -1px;
  }

  /* Tools Grid */
  .tools-grid {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .tool-card {
    background: var(--row);
    border: 1px solid var(--row-hi);
    border-radius: 12px;
    padding: 16px 12px;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    cursor: pointer;
    transition: all 0.2s;
  }

  .tool-card:hover {
    background: var(--row-hi);
    border-color: var(--accent);
    transform: translateY(-2px);
  }

  .tool-card span {
    font-size: 12px;
    color: var(--dim);
  }
</style>
