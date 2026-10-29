"use client";

import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import { Icon } from '@iconify/react';
import { VXLStudiosService } from '../../../services/vxl-studios-service';
import type { VxlCurseForgeCatalogMod } from '../../../services/vxl-api-client';
import {
  isVxlCurseForgeMod,
  isVxlCurseForgeModpack,
  isVxlCurseForgeResourcePack,
  latestVxlCurseForgeGameVersion,
  vxlCurseForgeProjectType,
} from '../../../services/vxl-curseforge-catalog';
import UnifiedService from '../../../services/unified-service';
import type { ModrinthSearchHit } from '../../../types/modrinth';
import type { ModrinthProject } from '../../../types/modrinth';
import { ModPlatform, UnifiedSortType, UnifiedProjectType, type UnifiedVersion } from '../../../types/unified';
import * as ProfileService from '../../../services/profile-service';
import { toast } from 'react-hot-toast';
import { ModrinthSearchControlsV2 } from '../../modrinth/v2/ModrinthSearchControlsV2';
import { ModrinthProjectCardV2 } from '../../modrinth/v2/ModrinthProjectCardV2';
import { ModrinthFilterSidebarV2 } from '../../modrinth/v2/ModrinthFilterSidebarV2';
import { useThemeStore } from '../../../store/useThemeStore';
import { Virtuoso } from 'react-virtuoso';
import { ContentType } from '../../../types/content';
import { useGlobalModal } from '../../../hooks/useGlobalModal';
import { ModrinthQuickInstallProfilesModal } from '../../modrinth/v2/ModrinthQuickInstallProfilesModal';

type Profile = any;

// Global cache for VXL Studios projects (persists for the launcher session)
const vxlStudiosCache = {
  modrinth: null as ModrinthProject[] | null,
  curseforge: null as VxlCurseForgeCatalogMod[] | null,
};

export interface VXLStudiosSearchV2Props {
  profiles: Profile[];
  onInstallSuccess?: () => void;
  className?: string;
  selectedProfileId?: string;
  initialSidebarVisible?: boolean;
}

export function VXLStudiosSearchV2({
  profiles: initialProfiles,
  onInstallSuccess,
  className = '',
  selectedProfileId,
  initialSidebarVisible = true,
}: VXLStudiosSearchV2Props) {
  const searchResultsAreaRef = useRef<HTMLDivElement>(null);
  
  const [searchTerm, setSearchTerm] = useState('');
  const [searchResults, setSearchResults] = useState<ModrinthSearchHit[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [showNoResultsMessage, setShowNoResultsMessage] = useState(false);
  const [loadingTimeout, setLoadingTimeout] = useState(false);
  const [activeSource, setActiveSource] = useState<'modrinth' | 'curseforge'>('modrinth');
  const [projectType, setProjectType] = useState<'mod' | 'modpack' | 'resourcepack'>('modpack');
  const [allModrinthProjects, setAllModrinthProjects] = useState<ModrinthProject[]>([]);
  const [allCurseForgeProjects, setAllCurseForgeProjects] = useState<VxlCurseForgeCatalogMod[]>([]);
  
  const [isSidebarVisible, setIsSidebarVisible] = useState(initialSidebarVisible);
  const [selectedProfile, setSelectedProfile] = useState<Profile | null>(null);
  const [internalProfiles, setInternalProfiles] = useState<Profile[]>(initialProfiles);
  
  const { accentColor } = useThemeStore();
  const { showModal, hideModal } = useGlobalModal();

  // Installation state management (matching ModrinthSearchV2)
  const [installing, setInstalling] = useState<Record<string, boolean>>({});
  const [installStatus, setInstallStatus] = useState<Record<string, boolean>>({});

  // Helper function to map project type to ContentType
  const mapProjectTypeToContentType = (projectType: string): ContentType | null => {
    switch (projectType) {
      case 'mod':
        return ContentType.Mod;
      case 'resourcepack':
        return ContentType.ResourcePack;
      case 'shader':
        return ContentType.ShaderPack;
      case 'datapack':
        return ContentType.DataPack;
      case 'modpack':
        toast.error("Modpacks should be installed as new profiles, not as content.");
        return null;
      default:
        console.warn(`Unsupported project type: ${projectType}`);
        return null;
    }
  };

  // Find the best version for a profile
  const findBestVersionForProfile = (profile: Profile, versions: UnifiedVersion[]): UnifiedVersion | null => {
    if (!profile || !versions || versions.length === 0) return null;
    
    // First try: find a version matching both game version and loader
    if (profile.game_version && profile.loader) {
      const exactMatch = versions.find(v => 
        v.game_versions.includes(profile.game_version) && 
        v.loaders.includes(profile.loader)
      );
      if (exactMatch) return exactMatch;
    }
    
    // Second try: match just game version
    if (profile.game_version) {
      const gameVersionMatch = versions.find(v => 
        v.game_versions.includes(profile.game_version)
      );
      if (gameVersionMatch) return gameVersionMatch;
    }
    
    // Last resort: return the latest version
    return versions[0];
  };

  // Quick install handler - shows profile selection modal
  const handleQuickInstall = async (project: ModrinthSearchHit & { gameVersion?: string | null }) => {
    // CurseForge is not supported in VXL Studios tab
    if (activeSource === 'curseforge') {
      toast.error("CurseForge installations are not supported in the VXL Studios tab. Please use the Mods tab instead.");
      return;
    }

    // Modpacks should not use quick install - they need to create new profiles
    if (project.project_type === 'modpack') {
      toast.error("Modpacks must be installed as new profiles. Please use the Mods tab for this functionality.");
      return;
    }

    const modalId = `vxl-quick-install-${project.project_id}`;

    try {
      // Check installation status for all profiles
      const statuses: Record<string, boolean> = {};
      for (const profile of internalProfiles) {
        try {
          const status = await ProfileService.isContentInstalled({
            profile_id: profile.id,
            project_id: project.project_id,
            project_type: project.project_type
          });
          statuses[profile.id] = !!status?.is_installed;
        } catch (err) {
          console.error(`Failed to check profile ${profile.name}:`, err);
          statuses[profile.id] = false;
        }
      }

      setInstallStatus(statuses);

      // Show the modal
      showModal(
        modalId,
        <ModrinthQuickInstallProfilesModal
          project={project}
          profiles={internalProfiles}
          onProfileSelect={handleProfileSelectionForQuickInstall}
          onClose={() => {
            hideModal(modalId);
            setInstallStatus({});
            setInstalling({});
          }}
          installingProfiles={installing}
          installStatus={statuses}
        />,
        1200
      );
    } catch (error) {
      console.error('Failed to open quick install modal:', error);
      toast.error('Failed to open installation dialog');
    }
  };

  // Handle profile selection and installation
  const handleProfileSelectionForQuickInstall = async (project: ModrinthSearchHit & { gameVersion?: string | null }, profile: Profile) => {
    // CurseForge installations are not supported in VXL Studios tab
    if (activeSource === 'curseforge') {
      toast.error("CurseForge installations are not supported in the VXL Studios tab. Please use the Mods tab instead.");
      setInstalling(prev => ({ ...prev, [profile.id]: false }));
      return;
    }

    // Set loading state for the profile being installed
    setInstalling(prev => ({ ...prev, [profile.id]: true }));

    try {
      // Fetch versions for this project
      const response = await UnifiedService.getModVersions({
        source: ModPlatform.Modrinth,
        project_id: project.project_id
      });

      if (!response.versions || response.versions.length === 0) {
        toast.error(`No versions found for ${project.title}`);
        setInstalling(prev => ({ ...prev, [profile.id]: false }));
        return;
      }

      // Find best version for selected profile
      const sortedVersions = response.versions.sort((a, b) => 
        new Date(b.date_published).getTime() - new Date(a.date_published).getTime()
      );
      const bestVersion = findBestVersionForProfile(profile, sortedVersions);

      if (!bestVersion) {
        toast.error(`No compatible version of ${project.title} for profile '${profile.name}'`);
        setInstalling(prev => ({ ...prev, [profile.id]: false }));
        return;
      }

      // Get primary file
      const primaryFile = bestVersion.files.find(f => f.primary) || bestVersion.files[0];
      if (!primaryFile) {
        toast.error(`No primary file found for ${project.title}`);
        setInstalling(prev => ({ ...prev, [profile.id]: false }));
        return;
      }

      // Install the content - use the project_type from the project
      await toast.promise(
        ProfileService.addModrinthContentToProfile(
          profile.id,
          project.project_id,
          bestVersion.id,
          primaryFile.filename,
          primaryFile.url,
          primaryFile.hashes?.sha1 || null,
          project.title,
          bestVersion.version_number,
          project.project_type
        ),
        {
          loading: `Installing ${project.title} to ${profile.name}...`,
          success: `Successfully installed ${project.title} to ${profile.name}`,
          error: (err) => `Failed to install: ${err.message || String(err)}`,
        }
      );

      // Update install status
      setInstallStatus(prev => ({ ...prev, [profile.id]: true }));

      if (onInstallSuccess) onInstallSuccess();
    } catch (error) {
      console.error('Installation failed:', error);
      toast.error(`Failed to install ${project.title}`);
    } finally {
      // Reset loading state for the profile
      setInstalling(prev => ({ ...prev, [profile.id]: false }));
    }
  };

  useEffect(() => {
    setInternalProfiles(initialProfiles);
    if (selectedProfileId && initialProfiles.length > 0) {
      const initiallySelectedProfile = initialProfiles.find(p => p.id === selectedProfileId);
      if (initiallySelectedProfile) {
        setSelectedProfile(initiallySelectedProfile);
      }
    }
  }, [initialProfiles, selectedProfileId]);

  // 10-second timeout for loading state
  useEffect(() => {
    if (loading) {
      const timeoutId = setTimeout(() => {
        setLoadingTimeout(true);
      }, 10000);
      return () => clearTimeout(timeoutId);
    } else {
      setLoadingTimeout(false);
    }
  }, [loading]);

  // Fetch Modrinth projects only when switching to Modrinth source (uses global cache)
  useEffect(() => {
    const fetchModrinthProjects = async () => {
      setLoading(true);
      setError(null);
      try {
        // Check global cache first
        if (vxlStudiosCache.modrinth) {
          setAllModrinthProjects(vxlStudiosCache.modrinth);
          console.log("[VXLStudiosSearchV2] Using cached Modrinth projects:", vxlStudiosCache.modrinth.length);
        } else {
          const modrinthProjects = await VXLStudiosService.getVXLStudiosModrinthProjects();
          vxlStudiosCache.modrinth = modrinthProjects;
          setAllModrinthProjects(modrinthProjects);
          console.log("[VXLStudiosSearchV2] Fetched fresh Modrinth projects:", modrinthProjects.length);
        }
      } catch (err) {
        console.error("[VXLStudiosSearchV2] Error fetching Modrinth projects:", err);
        setError("Failed to load Modrinth projects");
        setAllModrinthProjects([]);
      } finally {
        setLoading(false);
      }
    };

    if (activeSource === 'modrinth' && allModrinthProjects.length === 0) {
      fetchModrinthProjects();
    } else {
      setLoading(false);
    }
  }, [activeSource]);

  // Fetch CurseForge projects only when switching to CurseForge source (uses global cache)
  useEffect(() => {
    const fetchCurseForgeProjects = async () => {
      setLoading(true);
      setError(null);
      try {
        // Check global cache first
        if (vxlStudiosCache.curseforge) {
          setAllCurseForgeProjects(vxlStudiosCache.curseforge);
          console.log("[VXLStudiosSearchV2] Using cached CurseForge projects:", vxlStudiosCache.curseforge.length);
        } else {
          const curseForgeProjects =
            await VXLStudiosService.getVXLStudiosCurseForgeCatalog('minecraft');
          vxlStudiosCache.curseforge = curseForgeProjects;
          setAllCurseForgeProjects(curseForgeProjects);
          console.log(
            "[VXLStudiosSearchV2] Fetched fresh CurseForge projects:",
            curseForgeProjects.length,
          );
        }
      } catch (err) {
        console.error("[VXLStudiosSearchV2] Error fetching CurseForge projects:", err);
        setError("Failed to load CurseForge projects");
        setAllCurseForgeProjects([]);
      } finally {
        setLoading(false);
      }
    };

    if (activeSource === 'curseforge' && allCurseForgeProjects.length === 0) {
      fetchCurseForgeProjects();
    } else {
      setLoading(false);
    }
  }, [activeSource]);

  // Helper function to get latest game version from catalog CurseForge project
  const getLatestGameVersionCurseForge = (project: VxlCurseForgeCatalogMod): string | null =>
    latestVxlCurseForgeGameVersion(project);

  // Helper function to get latest game version from Modrinth project
  const getLatestGameVersionModrinth = (project: ModrinthProject): string | null => {
    if (!project.game_versions || project.game_versions.length === 0) {
      return null;
    }
    // Return the first (latest) game version
    return project.game_versions[0] || null;
  };

  // Filter and convert projects based on source and project type (client-side only)
  useEffect(() => {
    setError(null);
    setShowNoResultsMessage(false);
    try {
      let hits: (ModrinthSearchHit & { gameVersion?: string | null })[] = [];

      if (activeSource === 'modrinth') {
        // Filter cached Modrinth projects by project type (client-side only)
        const filteredProjects = allModrinthProjects.filter(proj => proj.project_type === projectType);
        
        // Convert ModrinthProject to ModrinthSearchHit format for compatibility
        hits = filteredProjects.map(proj => ({
          project_id: proj.id,
          project_type: proj.project_type,
          slug: proj.slug,
          title: proj.title,
          description: proj.description,
          author: null,
          categories: proj.categories,
          display_categories: proj.categories,
          client_side: proj.client_side,
          server_side: proj.server_side,
          downloads: proj.downloads,
          follows: proj.followers,
          icon_url: proj.icon_url,
          latest_version: null,
          date_created: proj.published,
          date_modified: proj.updated,
          license: proj.license.id,
          gallery: proj.gallery.map(g => g.url),
          versions: proj.versions,
          gameVersion: getLatestGameVersionModrinth(proj),
        }));
        
        setSearchResults(hits);
        if (hits.length === 0) {
          setShowNoResultsMessage(true);
        }
      } else if (activeSource === 'curseforge') {
        hits = allCurseForgeProjects
          .filter((proj) => {
            if (projectType === 'modpack') return isVxlCurseForgeModpack(proj);
            if (projectType === 'mod') return isVxlCurseForgeMod(proj);
            if (projectType === 'resourcepack') return isVxlCurseForgeResourcePack(proj);
            return true;
          })
          .map((proj) => ({
            project_id: proj.id.toString(),
            project_type: vxlCurseForgeProjectType(proj),
            slug: proj.slug,
            title: proj.name,
            description: proj.summary,
            author: null,
            categories: [],
            display_categories: [],
            client_side: 'required' as const,
            server_side: 'required' as const,
            downloads: proj.downloadCount,
            follows: 0,
            icon_url: proj.iconUrl ?? null,
            latest_version: null,
            date_created: '',
            date_modified: '',
            license: 'Unknown',
            gallery: [],
            versions: [],
            gameVersion: getLatestGameVersionCurseForge(proj),
          }));
        
        setSearchResults(hits);
        if (hits.length === 0) {
          setShowNoResultsMessage(true);
        }
      }
    } catch (err) {
      console.error("[VXLStudiosSearchV2] Error processing projects:", err);
      setError("Failed to process projects");
      setShowNoResultsMessage(true);
    }
  }, [activeSource, projectType, allModrinthProjects, allCurseForgeProjects]);

  // Filter results based on search term
  const filteredResults = useMemo(() => {
    if (!searchTerm) return searchResults;
    
    const lowerSearchTerm = searchTerm.toLowerCase();
    return searchResults.filter(hit =>
      hit.title.toLowerCase().includes(lowerSearchTerm) ||
      hit.description.toLowerCase().includes(lowerSearchTerm)
    );
  }, [searchResults, searchTerm]);

  const handleInstallSuccess = useCallback(() => {
    onInstallSuccess?.();
  }, [onInstallSuccess]);

  // Determine available project types for the current source
  const availableProjectTypes = useMemo(() => {
    const types: ('modpack' | 'mod' | 'resourcepack')[] = [];
    
    if (activeSource === 'modrinth') {
      // Modrinth has all types
      types.push('modpack', 'mod', 'resourcepack');
    } else if (activeSource === 'curseforge') {
      const hasModpacks = allCurseForgeProjects.some(isVxlCurseForgeModpack);
      const hasMods = allCurseForgeProjects.some(isVxlCurseForgeMod);
      const hasResourcepacks = allCurseForgeProjects.some(isVxlCurseForgeResourcePack);
      
      if (hasModpacks) types.push('modpack');
      if (hasMods) types.push('mod');
      if (hasResourcepacks) types.push('resourcepack');
    }
    
    return types;
  }, [activeSource, allCurseForgeProjects]);

  // Auto-switch to first available type if current type is not available
  useEffect(() => {
    if (availableProjectTypes.length > 0 && !availableProjectTypes.includes(projectType)) {
      setProjectType(availableProjectTypes[0]);
    }
  }, [availableProjectTypes, projectType]);

  return (
    <div className={`modrinth-search-v2 flex flex-row h-full gap-3 ${className}`}>
      {/* Left Content Area */}
      <div className="left-content-area flex flex-col flex-1 overflow-hidden">
        {/* Search controls */}
        <ModrinthSearchControlsV2
          searchTerm={searchTerm}
          onSearchTermChange={setSearchTerm}
          projectType={projectType}
          onProjectTypeChange={(type) => {
            if (type === 'modpack' || type === 'mod' || type === 'resourcepack') {
              setProjectType(type);
            }
          }}
          allProjectTypes={availableProjectTypes}
          profiles={internalProfiles}
          selectedProfile={selectedProfile}
          onSelectedProfileChange={(profile) => {
            setSelectedProfile(profile);
          }}
          sortOrder={UnifiedSortType.Relevance}
          onSortOrderChange={() => {}}
          sortOptions={[]}
          isSidebarVisible={isSidebarVisible}
          onToggleSidebar={() => setIsSidebarVisible(!isSidebarVisible)}
          selectedGameVersions={[]}
          currentSelectedLoaders={[]}
          currentSelectedCategories={[]}
          filterClientRequired={false}
          filterServerRequired={false}
          onRemoveGameVersionTag={() => {}}
          onRemoveLoaderTag={() => {}}
          onRemoveCategoryTag={() => {}}
          onRemoveClientRequiredTag={() => {}}
          onRemoveServerRequiredTag={() => {}}
          onClearAllFilters={() => {}}
          modSource={activeSource === 'curseforge' ? ModPlatform.CurseForge : ModPlatform.Modrinth}
          onModSourceChange={(source) => setActiveSource(source === ModPlatform.CurseForge ? 'curseforge' : 'modrinth')}
        />

        {/* Search Results Area */}
        <div ref={searchResultsAreaRef} className="search-results-area flex-1 overflow-y-auto">
          {searchResults.length === 0 && !loading && error && (
            <p className="p-4 text-red-500 text-center">Error: {error}</p>
          )}
          {searchResults.length === 0 && !loading && !error && showNoResultsMessage && (
            <div className="p-4 text-center flex flex-col items-center justify-center gap-4">
              <Icon icon="material-symbols:question-mark-rounded" className="text-6xl text-gray-400" />
              <p className="text-4xl lowercase text-gray-400">No Voxel Studios projects found.</p>
            </div>
          )}

          {searchResults.length > 0 && (
            <div className="space-y-1">
              {filteredResults.map((hit, index) => (
                <ModrinthProjectCardV2
                  key={hit.project_id}
                  itemIndex={index}
                  hit={hit}
                  accentColor={accentColor}
                  installStatus={null}
                  isQuickInstalling={installing[hit.project_id] || false}
                  isInstallingModpackAsProfile={false}
                  installingVersionStates={{}}
                  installingModpackVersionStates={{}}
                  onQuickInstallClick={handleQuickInstall}
                  onInstallModpackAsProfileClick={() => {}}
                  onInstallModpackVersionAsProfileClick={() => {}}
                  onToggleVersionsClick={() => {}}
                  isExpanded={false}
                  isLoadingVersions={false}
                  projectVersions={null}
                  displayedCount={0}
                  versionFilters={{ gameVersions: [], loaders: [], versionType: 'all' }}
                  versionDropdownUIState={{ showAllGameVersions: false, gameVersionSearchTerm: '' }}
                  openVersionDropdowns={{ type: false, gameVersion: false, loader: false }}
                  installedVersions={{}}
                  selectedProfile={selectedProfile}
                  selectedProfileId={selectedProfile?.id}
                  hoveredVersionId={null}
                  gameVersionsData={[]}
                  showAllGameVersionsSidebar={false}
                  selectedGameVersionsSidebar={[]}
                  onVersionFilterChange={() => {}}
                  onVersionUiStateChange={() => {}}
                  onToggleVersionDropdown={() => {}}
                  onCloseAllVersionDropdowns={() => {}}
                  onLoadMoreVersions={() => {}}
                  onInstallVersionClick={() => {}}
                  onHoverVersion={() => {}}
                  onDeleteVersionClick={() => {}}
                  onToggleEnableClick={() => {}}
                />
              ))}
            </div>
          )}

          {loading && !loadingTimeout && (
            <div className="p-4 text-center text-3xl text-gray-400">
              Loading Voxel Studios projects...
            </div>
          )}

          {loading && loadingTimeout && (
            <div className="p-4 text-center flex flex-col items-center justify-center gap-4">
              <Icon icon="material-symbols:question-mark-rounded" className="text-6xl text-gray-400" />
              <p className="text-4xl lowercase text-gray-400">No Voxel Studios projects found.</p>
            </div>
          )}
        </div>
      </div>

      {/* Filters Sidebar */}
      {isSidebarVisible && (
        <ModrinthFilterSidebarV2
          projectType="mod"
          accentColor={accentColor}
          gameVersionSearchTerm=""
          onGameVersionSearchTermChange={() => {}}
          displayedGameVersions={[]}
          selectedGameVersions={[]}
          onGameVersionToggle={() => {}}
          showAllGameVersionsSidebar={false}
          onShowAllGameVersionsSidebarChange={() => {}}
          availableLoaders={[]}
          currentSelectedLoaders={[]}
          onLoaderToggle={() => {}}
          allLoadersData={[]}
          dynamicFilterGroups={[]}
          currentSelectedCategories={[]}
          onCategoryToggle={() => {}}
          filterClientRequired={false}
          onClientRequiredToggle={() => {}}
          filterServerRequired={false}
          onServerRequiredToggle={() => {}}
          modSource={ModPlatform.Modrinth}
        />
      )}
    </div>
  );
}
