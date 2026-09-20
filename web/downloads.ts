interface Architectures<T> {
  x64?: T;
  arm64?: T;
}

interface LinuxPackages {
  deb?: string;
  appimage?: string;
}

interface Manifest {
  mac?: string;
  win?: Architectures<string>;
  linux?: Architectures<LinuxPackages>;
}

// Keep downloads available when the browser cannot start the hilen page.
async function showDownloads(): Promise<void> {
  const downloads = document.getElementById('downloads');
  if (!downloads) return;

  try {
    const response = await fetch('/download/manifest.json');
    if (!response.ok) return;
    const manifest: Manifest = await response.json();
    const choices: [string, string | undefined][] = [
      ['macOS', manifest.mac],
      ['Windows x64', manifest.win?.x64],
      ['Windows ARM64', manifest.win?.arm64],
      ['Linux x64 DEB', manifest.linux?.x64?.deb],
      ['Linux x64 AppImage', manifest.linux?.x64?.appimage],
      ['Linux ARM64 DEB', manifest.linux?.arm64?.deb],
      ['Linux ARM64 AppImage', manifest.linux?.arm64?.appimage],
    ];
    const links = document.createDocumentFragment();
    for (const [label, file] of choices) {
      if (typeof file !== 'string' || !/^blackforge-[\w.+-]+$/.test(file)) continue;
      const link = document.createElement('a');
      link.textContent = label;
      link.href = `https://gebling-studio.vladas.xyz/blackforge/download/${encodeURIComponent(file)}`;
      links.append(link, document.createElement('br'));
    }
    if (links.childNodes.length) downloads.replaceChildren(links);
  } catch (error) {
    console.warn('Could not load downloads', error);
  }
}

void showDownloads();
