<!--
  Workspace.svelte: System & Productivity Dashboard
  Displays network metrics, digital wellbeing log, and window management macros.
-->
<script>
  import { call } from '../lib/api.js';
</script>

<div class="workspace">
  <div class="placeholder">
    <h2>Workspace Dashboard</h2>
    <p>Coming soon in v2:</p>
    <ul>
      <li>🌐 Network Infrastructure Tracker</li>
      <li>⏱️ Digital Wellbeing Log</li>
      <li>📌 Window Management Macro</li>
    </ul>
  </div>
</div>

<style>
  .workspace {
    padding: 16px;
    height: 100%;
    overflow-y: auto;
  }
  .placeholder {
    text-align: center;
    padding: 40px 20px;
    color: #888;
  }
  .placeholder h2 {
    margin: 0 0 16px 0;
    font-size: 18px;
    color: #fff;
  }
  .placeholder p {
    margin: 0 0 12px 0;
  }
  .placeholder ul {
    list-style: none;
    padding: 0;
    margin: 0;
  }
  .placeholder li {
    padding: 8px 0;
    font-size: 14px;
  }
</style>
