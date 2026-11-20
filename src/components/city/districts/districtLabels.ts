// District labels for the Graph City.
// Translate a community DB name to a human-readable city district label
export function districtLabel(name: string, id: number): string {
  if (!name) return `DISTRICT ${id}`
  const map: Record<string, string> = {
    'agent-core': 'ARCANA DISTRICT',
    'api-health': 'API QUARTER',
    'memory-system': 'MEMORY SECTOR',
    'test-suite': 'TEST PRECINCT',
    'module-mcp': 'MCP TERMINAL',
  }
  const lower = name.toLowerCase()
  if (map[lower]) return map[lower]
  // A graph built before the naming fix carries a file path in the name
  // ("module-c:\work\src\city\a.ts"). Show the folder, not one label for every district.
  if (/[\\/]/.test(name)) {
    const parts = name.split(/[\\/]/).filter(Boolean)
    return (parts.length > 1 ? parts[parts.length - 2] : parts[0]).toUpperCase()
  }
  // Convert kebab-case community names to ALL CAPS label
  return name.toUpperCase().replace(/-/g, ' ')
}

/**
 * The label for every district, with a number added to any label two or more
 * districts would share (a graph built before the naming fix shows only the folder,
 * so many districts can read the same), so the labels can be told apart.
 */
export function districtLabels(
  districts: ReadonlyArray<{ communityId: number; name: string }>,
): Map<number, string> {
  const base = districts.map(d => [d.communityId, districtLabel(d.name, d.communityId)] as const)
  const counts = new Map<string, number>()
  for (const [, label] of base) counts.set(label, (counts.get(label) ?? 0) + 1)
  return new Map(
    base.map(([id, label]) => [id, (counts.get(label) ?? 0) > 1 ? `${label} #${id}` : label]),
  )
}
