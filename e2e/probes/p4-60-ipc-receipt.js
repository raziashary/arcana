// Fixed P4-60 renderer probe. This file is included only by the explicit
// `e2e-ipc-receipt` build feature; it accepts no remote or caller-supplied code.
(async () => {
  const invoke = (command, payload = {}) => {
    const runtime = window.__TAURI_INTERNALS__;
    if (!runtime?.invoke) throw new Error('Native Tauri IPC is unavailable.');
    return runtime.invoke(command, payload);
  };
  const waitFor = async (predicate, label, timeoutMs = 20_000) => {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      if (predicate()) return;
      await new Promise(resolve => setTimeout(resolve, 100));
    }
    throw new Error(`Timed out waiting for ${label}.`);
  };
  const clickButton = (label, root = document) => {
    const button = [...root.querySelectorAll('button')]
      .find(candidate => candidate.getAttribute('aria-label') === label || candidate.textContent.trim() === label);
    if (!button) throw new Error(`Button not found: ${label}`);
    button.click();
  };
  const clickTab = label => clickButton(label);
  const hasText = text => document.body.innerText.includes(text);
  let stage = 'renderer_start';

  try {
    await waitFor(() => document.querySelector('#root')?.children.length > 0, 'the real Arcana renderer');
    const fixture = await invoke('begin_e2e_ipc_receipt');
    stage = 'workspace_binding';
    await invoke('set_workspace_dir', { path: fixture.workspace });
    await invoke('set_active_provider', { provider: 'google', model: 'gemini-3.8-flash' });

    const privateDiff = `diff --git a/receipt.txt b/receipt.txt\nindex 1111111..2222222 100644\n--- a/receipt.txt\n+++ b/receipt.txt\n@@ -1 +1 @@\n-private baseline\n+P4_60_PRIVATE_DIFF_CANARY\n`;
    const claims = {
      renderer_ready: true,
      workspace_bound: false,
      consent_ui_granted: false,
      consent_ui_revoked: false,
      exact_provider_grant: false,
      cross_destination_denied: false,
      exact_provider_revoked: false,
      missing_and_revoked_diff_denied: false,
      readable_logs_visible: false,
      structured_event_visible: false,
      trace_visible: false,
      metrics_visible: false,
      export_ipc_returned: false,
      ide_navigation_opened: false,
      ide_run_output_visible: false,
    };

    stage = 'settings_controls';
    clickTab('Settings');
    await waitFor(() => hasText('Workspace egress consent'), 'Settings consent controls');
    await waitFor(() => document.querySelector('button[aria-label="Allow egress for GitHub MCP connector"]'), 'the ungranted GitHub MCP control');
    const selected = await invoke('get_workspace_dir');
    if (selected !== fixture.workspace) throw new Error('The renderer did not bind the isolated workspace.');
    claims.workspace_bound = true;

    stage = 'consent_flow';
    let denied = await invoke('analyze_diff', { diffText: privateDiff });
    if (denied.analysis_mode !== 'deterministic_only') throw new Error('Missing provider consent did not deny diff egress.');
    clickButton('Allow egress for GitHub MCP connector');
    await waitFor(() => document.querySelector('button[aria-label="Revoke consent for GitHub MCP connector"]'), 'granted consent in Settings');
    claims.consent_ui_granted = await invoke('get_workspace_egress_consent', { providerId: 'mcp:github-mcp' }) === true;
    claims.cross_destination_denied = await invoke('get_workspace_egress_consent', { providerId: 'google' }) === false;
    if (!claims.consent_ui_granted || !claims.cross_destination_denied) throw new Error('Consent was not scoped to its exact destination.');

    await invoke('grant_workspace_egress_consent', { providerId: 'google' });
    claims.exact_provider_grant = await invoke('get_workspace_egress_consent', { providerId: 'google' }) === true;
    if (!claims.exact_provider_grant) throw new Error('Exact provider grant did not read back.');
    await invoke('revoke_workspace_egress_consent', { providerId: 'google' });
    claims.exact_provider_revoked = await invoke('get_workspace_egress_consent', { providerId: 'google' }) === false;
    denied = await invoke('analyze_diff', { diffText: privateDiff });
    claims.missing_and_revoked_diff_denied = claims.exact_provider_revoked && denied.analysis_mode === 'deterministic_only';
    if (!claims.missing_and_revoked_diff_denied) throw new Error('Revoked provider consent did not force structural-only diff analysis.');

    clickButton('Revoke consent for GitHub MCP connector');
    await waitFor(() => document.querySelector('button[aria-label="Allow egress for GitHub MCP connector"]'), 'revoked consent in Settings');
    claims.consent_ui_revoked = await invoke('get_workspace_egress_consent', { providerId: 'mcp:github-mcp' }) === false;
    if (!claims.consent_ui_revoked) throw new Error('The Settings revoke was not read back from IPC.');

    stage = 'logs_tab';
    clickTab('Logs');
    await waitFor(() => document.querySelector('select[title="Select log date"]'), 'the production Logs tab');
    stage = 'logs_date';
    await waitFor(() => document.querySelector('select[title="Select log date"] option')?.value === fixture.date, 'the produced log date');
    stage = 'logs_readable';
    await waitFor(() => hasText('P4-60 IPC receipt readable producer'), 'the real readable log consumer');
    const logSections = [...document.querySelectorAll('details')];
    const section = label => logSections.find(item => item.querySelector('summary')?.textContent.includes(label));
    for (const label of ['Structured Events', 'Traces', 'Metrics']) {
      const target = section(label);
      if (!target) throw new Error(`Logs section not found: ${label}`);
      target.querySelector('summary').click();
    }
    stage = 'logs_structured';
    await waitFor(() => section('Structured Events')?.textContent.includes('p4_60_ipc_receipt_probe'), 'the real structured-event reader');
    const producedEvent = [...section('Structured Events').querySelectorAll('details')]
      .find(item => item.textContent.includes('p4_60_ipc_receipt_probe'));
    if (!producedEvent) throw new Error('Produced structured event is missing.');
    producedEvent.querySelector('summary').click();
    await waitFor(() => producedEvent.innerText.includes('p4_60_ipc_receipt_probe'), 'the expanded structured-event JSON');
    stage = 'logs_trace_metrics';
    claims.structured_event_visible = section('Structured Events').innerText.includes(fixture.run_id);
    claims.trace_visible = section('Traces').innerText.includes('p4_60_ipc_receipt_probe');
    await waitFor(() => section('Metrics').innerText.includes('operation_timing'), 'the timed-event metric breakdown');
    claims.metrics_visible = true;
    claims.readable_logs_visible = hasText('P4-60 IPC receipt readable producer');
    if (!claims.structured_event_visible || !claims.trace_visible || !claims.metrics_visible || !claims.readable_logs_visible) {
      throw new Error('One or more real Logs consumers did not read the produced evidence.');
    }

    stage = 'export';
    const exported = await invoke('export_observability_bundle', {
      date: fixture.date,
      destPath: fixture.export_path,
      correlationId: null,
    });
    claims.export_ipc_returned = exported === fixture.export_path;
    if (!claims.export_ipc_returned) throw new Error('Observability export IPC returned an unexpected path.');
    stage = 'ide_navigation';
    await invoke('open_in_ide', { file: `${fixture.workspace}\\src\\main.rs`, line: 1 });
    await waitFor(() => {
      const run = [...document.querySelectorAll('button')].find(button => button.textContent.trim() === 'Run');
      return hasText('main.rs') && run && !run.disabled;
    }, 'the IDE event consumer and opened artifact');
    claims.ide_navigation_opened = true;
    stage = 'ide_run';
    clickButton('Run');
    await waitFor(() => Boolean(document.querySelector('[aria-label="Run output"]'))
      || Boolean(document.querySelector('[aria-label="IDE run result"] [role="alert"]')),
    'the native IDE Run result', 120_000);
    const runOutput = document.querySelector('[aria-label="Run output"]')?.textContent ?? '';
    if (!runOutput.includes('exit 0') || !runOutput.includes('P4_21_IDE_RUN_OK')) {
      throw new Error('The native IDE Run did not complete with the fixed artifact output.');
    }
    claims.ide_run_output_visible = true;
    await invoke('finish_e2e_ipc_receipt', { rendererReceipt: JSON.stringify(claims) });
  } catch (error) {
    console.error('P4-60 isolated IPC receipt failed at stage:', stage);
    const message = String(error);
    const reason = document.body.innerText.includes('Logs crashed') ? 'logs_crashed'
      : stage === 'logs_tab' && hasText('Workspace egress consent') ? 'settings_still_visible'
        : stage === 'logs_tab' ? 'logs_panel_missing'
          : message.startsWith('Error: Button not found:') ? 'button_absent'
            : message.startsWith('Error: Timed out waiting for') ? 'timeout'
              : message.includes('invoke') ? 'invoke_error' : 'unexpected';
    await invoke('fail_e2e_ipc_receipt', { stage, reason }).catch(() => {});
  }
})();
