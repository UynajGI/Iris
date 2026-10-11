import { useEffect, useState } from 'react';
import { usePersisted } from './usePersisted.js';

/** Panel preferences survive page changes; the narrow library is only a drawer. */
export function useWorkspacePanels() {
  const [leftOpen, setLeftOpen] = usePersisted<boolean>('iris:left-panel:v1', true);
  const [rightOpen, setRightOpen] = usePersisted<boolean>('iris:right-panel:v1', true);
  const [stripOpen, setStripOpen] = usePersisted<boolean>('iris:filmstrip:v1', true);
  const [showFaces, setShowFaces] = usePersisted<boolean>('iris:face-overlay:v1', false);
  const [tileInfo, setTileInfo] = usePersisted<boolean>('iris:tile-info:v1', true);
  const [wide, setWide] = useState(() => window.matchMedia('(min-width: 1151px)').matches);
  const [libraryDrawer, setLibraryDrawer] = useState(false);
  const libraryOpen = wide ? leftOpen : libraryDrawer;
  const toggleLibrary = () => wide ? setLeftOpen(!leftOpen) : setLibraryDrawer(!libraryDrawer);
  useEffect(() => {
    const query = window.matchMedia('(min-width: 1151px)');
    const update = () => { setWide(query.matches); setLibraryDrawer(false); };
    query.addEventListener('change', update);
    return () => query.removeEventListener('change', update);
  }, []);
  return { setLeftOpen, rightOpen, setRightOpen, stripOpen, setStripOpen, showFaces, setShowFaces, tileInfo, setTileInfo, wide, libraryDrawer, setLibraryDrawer, libraryOpen, toggleLibrary };
}
