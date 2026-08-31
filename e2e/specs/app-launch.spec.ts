// The first end-to-end scenario: the built application starts and renders.
//
// Trivial on purpose. Its job in P0 is to prove the harness itself works —
// that tauri-driver launches the real binary, the webview is reachable, and an
// assertion can fail. The acceptance test of `10 §2` is built on top of this in
// P9; a harness that first exists in P9 cannot gate anything before it.
//
// It is still not vacuous: AutoDev opens a database, installs a process-wide
// AppHandle and starts background tasks during setup. A panic in any of that
// leaves no window for the driver to attach to, and this fails.

describe('AutoDev application launch', () => {
  it('opens a window and renders the React root', async () => {
    const root = await $('#root')
    await root.waitForExist({ timeout: 30_000 })

    // An empty #root means the bundle loaded but React never mounted — a
    // white-screen failure that a "window opened" check alone would miss.
    await browser.waitUntil(
      async () => (await root.getHTML(false)).trim().length > 0,
      {
        timeout: 30_000,
        timeoutMsg: 'The window opened but #root stayed empty — React did not mount.',
      },
    )
  })

  it('reports a document title', async () => {
    const title = await browser.getTitle()
    expect(title).not.toBe('')
  })
})
