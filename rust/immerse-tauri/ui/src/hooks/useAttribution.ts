import attributionData from '../data/attribution.json';

export interface Attribution {
  name: string;
  author: string;
  license: string;
  url: string;
}

const attributionMap = attributionData as Record<string, Attribution>;

/** Look up attribution for a freesound URL. Returns null for CC0 / unknown sounds. */
export function getAttribution(url: string): Attribution | null {
  return attributionMap[url] ?? null;
}

/** Get all attributions grouped by author, sorted alphabetically. */
export function getAttributionsByAuthor(): Record<string, Attribution[]> {
  const byAuthor: Record<string, Attribution[]> = {};
  for (const attr of Object.values(attributionMap)) {
    if (!byAuthor[attr.author]) {
      byAuthor[attr.author] = [];
    }
    byAuthor[attr.author].push(attr);
  }
  // Sort authors alphabetically (case-insensitive)
  const sorted: Record<string, Attribution[]> = {};
  for (const author of Object.keys(byAuthor).sort((a, b) => a.localeCompare(b, undefined, { sensitivity: 'base' }))) {
    sorted[author] = byAuthor[author].sort((a, b) => a.name.localeCompare(b.name));
  }
  return sorted;
}

/** Format a license string for display (e.g., "CC-BY-4.0" -> "CC BY 4.0") */
export function formatLicense(license: string): string {
  return license.replace(/-/g, ' ').replace('CC BY', 'CC-BY');
}
