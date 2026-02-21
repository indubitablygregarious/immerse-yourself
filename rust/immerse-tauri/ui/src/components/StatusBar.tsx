import type { FC } from 'react';
import type { ActiveState } from '../types';
import { getAttribution, formatLicense } from '../hooks/useAttribution';

interface StatusBarProps {
  activeState: ActiveState | null;
}

export const StatusBar: FC<StatusBarProps> = ({ activeState }) => {
  const activeSound = activeState?.active_sound ?? null;
  const soundStatus = activeSound ?? 'idle';

  // Get atmosphere names or count
  const atmosphereNames = activeState?.atmosphere_names ?? [];
  const atmosphereNamesWithAuthor = activeState?.atmosphere_names_with_author ?? [];
  const atmosphereCount = atmosphereNames.length;
  const musicStatus = atmosphereCount > 0
    ? `Atmosphere: ${atmosphereNames.slice(0, 2).join(' + ')}${atmosphereCount > 2 ? ` +${atmosphereCount - 2}` : ''}`
    : 'none';

  const lightsStatus = activeState?.active_lights_config || 'none';

  const isDownloading = activeState?.is_downloading ?? false;
  const downloadCount = activeState?.pending_downloads ?? 0;

  // Build detailed tooltip like Python app
  const tooltipLines: string[] = [];

  // Sound section
  tooltipLines.push('═══ Sound ═══');
  if (activeSound) {
    tooltipLines.push(`  🔊 ${activeSound}`);
  } else {
    tooltipLines.push('  (not playing)');
  }
  tooltipLines.push('');

  // Atmosphere section - show names with author info
  tooltipLines.push('═══ Atmosphere ═══');
  const activeUrls = activeState?.active_atmosphere_urls ?? [];
  if (atmosphereNamesWithAuthor.length > 0) {
    for (let i = 0; i < atmosphereNamesWithAuthor.length; i++) {
      tooltipLines.push(`  🌊 ${atmosphereNamesWithAuthor[i]}`);
      // Show CC-BY attribution if applicable
      const url = activeUrls[i];
      if (url) {
        const attr = getAttribution(url);
        if (attr) {
          tooltipLines.push(`       by ${attr.author} — ${formatLicense(attr.license)}`);
        }
      }
    }
  } else {
    tooltipLines.push('  (not playing)');
  }
  tooltipLines.push('');

  // Lights section
  tooltipLines.push('═══ Lights ═══');
  if (lightsStatus !== 'none') {
    tooltipLines.push(`  💡 ${lightsStatus}`);
  } else {
    tooltipLines.push('  (not active)');
  }

  // Downloads section (if any)
  if (downloadCount > 0) {
    tooltipLines.push('');
    tooltipLines.push('═══ Downloads ═══');
    tooltipLines.push(`  📥 ${downloadCount} pending`);
  }

  const tooltipText = tooltipLines.join('\n');

  return (
    <div className="status-bar" title={tooltipText}>
      <span className="status-brand">Immerse yourself running</span>
      <span className="status-separator">||</span>
      <span className="status-item">
        <span className="status-label">sound:</span>
        <span className="status-value">{soundStatus}</span>
      </span>
      <span className="status-separator">||</span>
      <span className="status-item">
        <span className="status-label">music:</span>
        <span className="status-value">{musicStatus}</span>
      </span>
      <span className="status-separator">||</span>
      <span className="status-item">
        <span className="status-label">lights:</span>
        <span className="status-value">{lightsStatus}</span>
      </span>
      {isDownloading && (
        <>
          <span className="status-separator">||</span>
          <span className="status-item downloading">
            <span className="status-label">downloading:</span>
            <span className="status-value">{downloadCount}</span>
          </span>
        </>
      )}
    </div>
  );
};
