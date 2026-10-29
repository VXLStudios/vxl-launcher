"use client";

import { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import UnifiedService from '../../../../services/unified-service';
import { ModrinthService } from '../../../../services/modrinth-service';
import { VXLStudiosService } from '../../../../services/vxl-studios-service';
import { vxlCurseForgeProjectType } from '../../../../services/vxl-curseforge-catalog';
import type {
  UnifiedModSearchResult,
  UnifiedModSearchResponse,
  UnifiedVersion,
} from '../../../../types/unified';
import { ModPlatform, UnifiedSortType } from '../../../../types/unified';
import type {
  ModrinthProjectType,
  ModrinthCategory,
  ModrinthGameVersion,
  ModrinthLoader,
} from '../../../../types/modrinth';
import { useThemeStore } from '../../../../store/useThemeStore';
import { useDisplayContextStore } from '../../../../store/useDisplayContextStore';
import {
  ALL_MODRINTH_PROJECT_TYPES,
  PREFERRED_HEADER_ORDER,
  INITIAL_DISPLAY_COUNT,
  LOAD_MORE_INCREMENT,
  SEARCH_PAGE_LIMIT,
  convertToUnifiedProjectType,
  type Profile,
  type UIDynamicFilterGroup,
} from '../modrinthSearchShared';
import { debugLog } from '../utils/debug';
import type { VersionFiltersState } from './useModrinthUpdates';

export interface UseModrinthSearchParams {
  profiles: Profile[];
  selectedProfileId?: string;
  initialSidebarVisible?: boolean;
  overrideDisplayContext?: 'detail' | 'standalone';
  initialProjectType?: ModrinthProjectType;
  allowedProjectTypes?: ModrinthProjectType[];
  useVXLStudiosData?: boolean;
  checkDisplayedVersionsStatus?: (
    projectId: string,
    versions: UnifiedVersion[],
    startIndex: number,
    count: number,
    forceRefresh?: string[],
  ) => Promise<void>;
}

export function useModrinthSearch({
  profiles: initialProfiles,
  selectedProfileId,
  initialSidebarVisible = true,
  overrideDisplayContext,
  initialProjectType,
  allowedProjectTypes,
  useVXLStudiosData = false,
  checkDisplayedVersionsStatus,
}: UseModrinthSearchParams) {
  const checkDisplayedVersionsStatusRef = useRef(checkDisplayedVersionsStatus);
  checkDisplayedVersionsStatusRef.current = checkDisplayedVersionsStatus;
  const searchResultsAreaRef = useRef<HTMLDivElement>(null);
  const [searchTerm, setSearchTerm] = useState('');
  const [projectType, setProjectType] = useState<ModrinthProjectType>(() => {
    const effectiveAllowedTypes = allowedProjectTypes || ALL_MODRINTH_PROJECT_TYPES;
    if (initialProjectType && effectiveAllowedTypes.includes(initialProjectType)) {
      return initialProjectType;
    }
    return effectiveAllowedTypes[0] || 'mod'; // Default to first allowed type or 'mod'
  });
  const [searchResults, setSearchResults] = useState<UnifiedModSearchResult[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [offset, setOffset] = useState(0);
  const [totalHits, setTotalHits] = useState(0);
  const limit = SEARCH_PAGE_LIMIT;
  
  // State to control delayed display of "No results found" message
  const [showNoResultsMessage, setShowNoResultsMessage] = useState(false);
  const noResultsTimeoutRef = useRef<NodeJS.Timeout | null>(null);

  // New state for Sort Order, now with UnifiedSortType
  const [sortOrder, setSortOrder] = useState<UnifiedSortType>(UnifiedSortType.Relevance);
  
  const sortOptions: { value: UnifiedSortType; label: string; icon?: string }[] = [
    { value: UnifiedSortType.Relevance, label: 'Relevance', icon: 'solar:sort-bold' },
    { value: UnifiedSortType.Downloads, label: 'Downloads', icon: 'solar:download-bold' },
    { value: UnifiedSortType.Follows, label: 'Follows', icon: 'solar:heart-bold' },
    { value: UnifiedSortType.Newest, label: 'Newest', icon: 'solar:calendar-mark-bold' },
    { value: UnifiedSortType.Updated, label: 'Updated', icon: 'solar:refresh-bold' },
  ];

  const [allCategoriesData, setAllCategoriesData] = useState<ModrinthCategory[]>([]);
  const [gameVersionsData, setGameVersionsData] = useState<ModrinthGameVersion[]>([]);
  const [allLoadersData, setAllLoadersData] = useState<ModrinthLoader[]>([]);

  const initialCategoriesState = useMemo(() => 
    (allowedProjectTypes || ALL_MODRINTH_PROJECT_TYPES).reduce((acc, pt) => ({ ...acc, [pt]: [] }), {} as Record<ModrinthProjectType, string[]>)
  , [allowedProjectTypes]);
  const [selectedCategoriesByProjectType, setSelectedCategoriesByProjectType] = useState(initialCategoriesState);
  
  const [selectedLoadersByProjectType, setSelectedLoadersByProjectType] = useState(initialCategoriesState);
  
  const [selectedGameVersions, setSelectedGameVersions] = useState<string[]>([]); 
  const [showAllGameVersionsSidebar, setShowAllGameVersionsSidebar] = useState(false); // Renamed state and set default to false
  const [gameVersionSearchTerm, setGameVersionSearchTerm] = useState('');

  // New states for Environment filter
  const [filterClientRequired, setFilterClientRequired] = useState(false);
  const [filterServerRequired, setFilterServerRequired] = useState(false);

  // New state for expanded versions
  const [expandedVersions, setExpandedVersions] = useState<Record<string, UnifiedVersion[] | null | 'loading'>>({});

  // New state for managing how many versions are displayed per project
  const [numDisplayedVersions, setNumDisplayedVersions] = useState<Record<string, number>>({});
  const initialDisplayCount = INITIAL_DISPLAY_COUNT;
  const loadMoreIncrement = LOAD_MORE_INCREMENT;

  // New state for version filtering
  const [versionFilters, setVersionFilters] = useState<Record<string, {
    gameVersions: string[],
    loaders: string[],
    versionType: string
  }>>({});

  // Add new state for sidebar visibility
  const [isSidebarVisible, setIsSidebarVisible] = useState(initialSidebarVisible);

  // Add state for currently selected profile
  const [selectedProfile, setSelectedProfile] = useState<Profile | null>(null);

  // Get mod source from theme store (persistent)
  const { modSource, setModSource } = useThemeStore();

  // Internal state for profiles, synced with the prop
  const [internalProfiles, setInternalProfiles] = useState<Profile[]>(initialProfiles);

  useEffect(() => {
    setInternalProfiles(initialProfiles);
    // If a selectedProfileId is passed as a prop, find and set it.
    if (selectedProfileId && initialProfiles.length > 0) {
      const initiallySelectedProfile = initialProfiles.find(p => p.id === selectedProfileId);
      if (initiallySelectedProfile) {
        setSelectedProfile(initiallySelectedProfile);
      }
    }
  }, [initialProfiles, selectedProfileId]);

  const currentSelectedCategories = useMemo(() => {
    return selectedCategoriesByProjectType[projectType] || [];
  }, [selectedCategoriesByProjectType, projectType]);

  const currentSelectedLoaders = useMemo(() => {
    return selectedLoadersByProjectType[projectType] || [];
  }, [selectedLoadersByProjectType, projectType]);

  // Fetch filter data on mount
  useEffect(() => {
    const fetchFilterData = async () => {
      try {
        setAllCategoriesData(await ModrinthService.getModrinthCategories());
        setGameVersionsData(await ModrinthService.getModrinthGameVersions());
        setAllLoadersData(await ModrinthService.getModrinthLoaders());
      } catch (err) { console.error("Failed to load filter data:", err); }
    };
    fetchFilterData();
  }, []);

  // Store all VXL Studios projects for filtering
  const [allVXLStudiosProjects, setAllVXLStudiosProjects] = useState<UnifiedModSearchResult[]>([]);

  // Global cache for VXL Studios data (session-level, cleared on app restart)
  const vxlStudiosCache = useMemo(() => {
    if (typeof window !== 'undefined' && !(window as any).vxlStudiosCache) {
      (window as any).vxlStudiosCache = {
        modrinth: null as UnifiedModSearchResult[] | null,
        curseforge: null as UnifiedModSearchResult[] | null,
      };
    }
    return (window as any).vxlStudiosCache || { modrinth: null, curseforge: null };
  }, []);

  // Load VXL Studios data if useVXLStudiosData prop is true
  useEffect(() => {
    if (!useVXLStudiosData) return;

    const loadVXLStudiosData = async () => {
      setLoading(true);
      try {
        debugLog('[ModrinthSearchV2] Loading VXL Studios data for source:', modSource);
        let convertedProjects: UnifiedModSearchResult[] = [];

        if (modSource === ModPlatform.Modrinth) {
          // Check cache first
          if (vxlStudiosCache.modrinth) {
            debugLog('[ModrinthSearchV2] Using cached Modrinth VXL Studios data');
            convertedProjects = vxlStudiosCache.modrinth;
          } else {
            const modrinthProjects = await VXLStudiosService.getVXLStudiosModrinthProjects();
            
            // Convert ModrinthProject objects to UnifiedModSearchResult format
            convertedProjects = modrinthProjects.map(proj => ({
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
              source: ModPlatform.Modrinth,
              project_url: `https://modrinth.com/${proj.project_type}/${proj.slug}`,
            }));
            
            // Cache the results
            vxlStudiosCache.modrinth = convertedProjects;
            debugLog('[ModrinthSearchV2] Cached Modrinth VXL Studios data');
          }
        } else if (modSource === ModPlatform.CurseForge) {
          // Check cache first
          if (vxlStudiosCache.curseforge) {
            debugLog('[ModrinthSearchV2] Using cached CurseForge VXL Studios data');
            convertedProjects = vxlStudiosCache.curseforge;
          } else {
            // Fetch CurseForge mod IDs from VXL Studios API
            try {
              const catalog = await VXLStudiosService.getVXLStudiosCurseForgeCatalog('minecraft');
              debugLog('[ModrinthSearchV2] CurseForge catalog from Studios API:', catalog.length);

              convertedProjects = catalog.map((proj) => {
                const mappedType = vxlCurseForgeProjectType(proj);
                return {
                  project_id: proj.id.toString(),
                  project_type: mappedType,
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
                  source: ModPlatform.CurseForge,
                  project_url: proj.websiteUrl,
                };
              });
              
              // Cache the results
              vxlStudiosCache.curseforge = convertedProjects;
              debugLog('[ModrinthSearchV2] Cached CurseForge VXL Studios data');
            } catch (err) {
              console.error('[ModrinthSearchV2] Failed to load CurseForge VXL Studios projects:', err);
              convertedProjects = [];
            }
          }
        }

        debugLog('[ModrinthSearchV2] Loaded VXL Studios projects:', convertedProjects.length);
        setAllVXLStudiosProjects(convertedProjects);
        setSearchResults(convertedProjects);
        setTotalHits(convertedProjects.length);
        setOffset(convertedProjects.length);
      } catch (err) {
        console.error('[ModrinthSearchV2] Failed to load VXL Studios data:', err);
        setError(`Failed to load VXL Studios projects: ${err instanceof Error ? err.message : String(err)}`);
      } finally {
        setLoading(false);
      }
    };

    loadVXLStudiosData();
  }, [useVXLStudiosData, modSource, vxlStudiosCache]);

  // Calculate available project types (only show types that have content)
  const availableProjectTypes = useMemo(() => {
    if (!useVXLStudiosData || allVXLStudiosProjects.length === 0) {
      return allowedProjectTypes || ALL_MODRINTH_PROJECT_TYPES;
    }
    
    // Get unique project types from VXL Studios data
    const typesInData = new Set(allVXLStudiosProjects.map(proj => proj.project_type));
    
    // Filter allowedProjectTypes to only include types that exist in data
    const filtered = (allowedProjectTypes || ALL_MODRINTH_PROJECT_TYPES).filter(type => typesInData.has(type));
    
    return filtered.length > 0 ? filtered : (allowedProjectTypes || ALL_MODRINTH_PROJECT_TYPES);
  }, [useVXLStudiosData, allVXLStudiosProjects, allowedProjectTypes]);

  // Default to modpacks if current project type is not available
  useEffect(() => {
    if (!useVXLStudiosData || availableProjectTypes.length === 0) return;
    
    // If current projectType is not in availableProjectTypes, switch to modpacks
    if (!availableProjectTypes.includes(projectType)) {
      debugLog('[ModrinthSearchV2] Current project type not available, defaulting to modpacks');
      setProjectType('modpack');
    }
  }, [useVXLStudiosData, availableProjectTypes, projectType]);

  // Filter VXL Studios projects by project type, search term, and all other filters
  useEffect(() => {
    if (!useVXLStudiosData || allVXLStudiosProjects.length === 0) return;

    let filtered = allVXLStudiosProjects;

    // Filter by project type
    filtered = filtered.filter(proj => proj.project_type === projectType);

    // Filter by search term
    if (searchTerm) {
      const lowerSearchTerm = searchTerm.toLowerCase();
      filtered = filtered.filter(proj =>
        proj.title.toLowerCase().includes(lowerSearchTerm) ||
        proj.description.toLowerCase().includes(lowerSearchTerm)
      );
    }

    // Filter by categories
    if (currentSelectedCategories.length > 0) {
      filtered = filtered.filter(proj =>
        currentSelectedCategories.some(cat => proj.categories?.includes(cat))
      );
    }

    // Filter by game versions
    if (selectedGameVersions.length > 0) {
      filtered = filtered.filter(proj => {
        if (!Array.isArray(proj.versions)) return false;
        
        // Check if versions are objects (Modrinth) or strings (CurseForge)
        return proj.versions.some((v: any) => {
          if (typeof v === 'string') {
            // CurseForge version IDs - can't filter by game versions, skip these projects
            return false;
          }
          // Modrinth version objects - can filter by game_versions
          return selectedGameVersions.some(gv => (v as any).game_versions?.includes(gv));
        });
      });
    }

    // Filter by loaders
    if (currentSelectedLoaders.length > 0) {
      filtered = filtered.filter(proj => {
        if (!Array.isArray(proj.versions)) return false;
        
        // Check if versions are objects (Modrinth) or strings (CurseForge)
        return proj.versions.some((v: any) => {
          if (typeof v === 'string') {
            // CurseForge version IDs - can't filter by loaders, skip these projects
            return false;
          }
          // Modrinth version objects - can filter by loaders
          return currentSelectedLoaders.some(loader => (v as any).loaders?.includes(loader));
        });
      });
    }

    // Filter by environment (client/server required)
    if (filterClientRequired) {
      filtered = filtered.filter(proj => proj.client_side === 'required');
    }
    if (filterServerRequired) {
      filtered = filtered.filter(proj => proj.server_side === 'required');
    }

    debugLog('[ModrinthSearchV2] Filtered VXL Studios projects:', filtered.length, 'by type:', projectType, 'search:', searchTerm, 'categories:', currentSelectedCategories, 'gameVersions:', selectedGameVersions, 'loaders:', currentSelectedLoaders);
    setSearchResults(filtered);
    setTotalHits(filtered.length);
    setOffset(filtered.length);
  }, [useVXLStudiosData, allVXLStudiosProjects, projectType, searchTerm, currentSelectedCategories, selectedGameVersions, currentSelectedLoaders, filterClientRequired, filterServerRequired]);

  // Define preferred loader order
  const preferredLoaderOrder = ['fabric', 'forge', 'quilt', 'neoforge'];

  const availableLoaders = useMemo(() => {
    const loaders = allLoadersData.filter(loader => loader.supported_project_types.includes(projectType));
    // Sort loaders: preferred first, then alphabetical
    return loaders.sort((a, b) => {
      const indexA = preferredLoaderOrder.indexOf(a.name.toLowerCase());
      const indexB = preferredLoaderOrder.indexOf(b.name.toLowerCase());

      if (indexA !== -1 && indexB !== -1) return indexA - indexB; // Both preferred
      if (indexA !== -1) return -1; // Only A is preferred
      if (indexB !== -1) return 1; // Only B is preferred
      return a.name.localeCompare(b.name); // Neither preferred, sort alphabetically
    });
  }, [allLoadersData, projectType]);

  const displayedGameVersions = useMemo(() => {
    let versions = gameVersionsData;
    // Inverted logic: Only filter for release if showAllGameVersionsSidebar is FALSE
    if (!showAllGameVersionsSidebar) { 
      versions = versions.filter(gv => gv.version_type === 'release'); 
    }
    if (gameVersionSearchTerm) { 
      versions = versions.filter(gv => gv.version.toLowerCase().includes(gameVersionSearchTerm.toLowerCase()));
    }
    return versions;
  }, [gameVersionsData, showAllGameVersionsSidebar, gameVersionSearchTerm]); // Use new state here

  // Dynamically generate filter groups based on headers for the current project type
  // Note: CurseForge doesn't support category filtering, so we only show categories for Modrinth
  const dynamicFilterGroups = useMemo<UIDynamicFilterGroup[]>(() => {
    // Don't show categories for CurseForge as it doesn't support them properly
    if (modSource === ModPlatform.CurseForge || !allCategoriesData.length || !projectType) return [];

    const categoriesForProjectType = allCategoriesData.filter(cat => cat.project_type === projectType);
    const headers = [...new Set(categoriesForProjectType.map(cat => cat.header))];

    const groups = headers.map(header => {
      const optionsForHeader = categoriesForProjectType.filter(cat => cat.header === header);
      // Simple title generation: capitalize first letter, replace hyphens
      const accordionTitle = header.charAt(0).toUpperCase() + header.slice(1).replace(/-/g, ' ');
      return {
        accordionTitle,
        headerValue: header,
        options: optionsForHeader.sort((a, b) => a.name.localeCompare(b.name)), // Sort options alphabetically
      };
    });

    // Sort the groups themselves
    return groups.sort((a, b) => {
      const lowerA = a.headerValue.toLowerCase();
      const lowerB = b.headerValue.toLowerCase();
      const indexA = PREFERRED_HEADER_ORDER.indexOf(lowerA);
      const indexB = PREFERRED_HEADER_ORDER.indexOf(lowerB);

      if (indexA !== -1 && indexB !== -1) return indexA - indexB;
      if (indexA !== -1) return -1;
      if (indexB !== -1) return 1;
      return a.accordionTitle.localeCompare(b.accordionTitle);
    });
  }, [allCategoriesData, projectType, modSource]);


  // Global cache for regular search results (session-level, cleared on app restart)
  const searchCache = useMemo(() => {
    if (typeof window !== 'undefined' && !(window as any).modrinthSearchCache) {
      (window as any).modrinthSearchCache = {};
    }
    return (window as any).modrinthSearchCache || {};
  }, []);

  // Generate cache key based on search parameters
  const getCacheKey = useCallback((
    query: string,
    source: ModPlatform,
    type: ModrinthProjectType,
    categories: string[],
    gameVersion: string | undefined,
    loaders: string[],
    sortBy: UnifiedSortType
  ): string => {
    return `${source}:${type}:${query}:${categories.join(',')}:${gameVersion || 'any'}:${loaders.join(',')}:${sortBy}`;
  }, []);

  const performSearch = useCallback(async (newSearch = false) => {
    debugLog('[ModrinthSearchV2] performSearch ENTRY:', {
      newSearch,
      projectType,
      searchTerm,
      categories: currentSelectedCategories,
      gameVersions: selectedGameVersions,
      loaders: currentSelectedLoaders,
      offset: newSearch ? 0 : offset // Log the offset that will be used
    });

    // Clear any existing timeout for the "No results found" message
    if (noResultsTimeoutRef.current) {
      clearTimeout(noResultsTimeoutRef.current);
      noResultsTimeoutRef.current = null;
    }
    setShowNoResultsMessage(false);

    if (newSearch) {
      debugLog('[ModrinthSearchV2] New search, resetting offset.');
      setOffset(0);
      // setSearchResults([]); // DO NOT clear previous results here to prevent flicker
    }

    debugLog('[ModrinthSearchV2] Proceeding with API call. Setting loading true.');
    setLoading(true);
    setError(null);

    try {
      // Generate cache key for this search
      const cacheKey = getCacheKey(
        searchTerm,
        modSource,
        projectType,
        currentSelectedCategories,
        selectedGameVersions.length > 0 ? selectedGameVersions[0] : undefined,
        currentSelectedLoaders,
        sortOrder
      );

      // Check cache for first page (offset 0)
      if (newSearch && searchCache[cacheKey]) {
        debugLog('[ModrinthSearchV2] Using cached search results for key:', cacheKey);
        const cachedResponse = searchCache[cacheKey];
        setSearchResults(cachedResponse.results);
        setTotalHits(cachedResponse.pagination.total_count);
        setOffset(cachedResponse.results.length);
        setLoading(false);
        return;
      }

      const response: UnifiedModSearchResponse = await UnifiedService.searchMods({
        query: searchTerm,
        source: modSource,
        project_type: convertToUnifiedProjectType(projectType),
        game_version: selectedGameVersions.length > 0 ? selectedGameVersions[0] : undefined,
        mod_loaders: currentSelectedLoaders.length > 0 ? currentSelectedLoaders : undefined,
        limit,
        offset: newSearch ? 0 : offset,
        sort: sortOrder as UnifiedSortType,
        categories: currentSelectedCategories.length > 0 ? currentSelectedCategories : undefined,
        client_side_filter: filterClientRequired ? "required" : undefined,
        server_side_filter: filterServerRequired ? "required" : undefined
      });

      // Cache the first page results
      if (newSearch) {
        searchCache[cacheKey] = response;
        debugLog('[ModrinthSearchV2] Cached search results for key:', cacheKey);
      }

      setSearchResults(prevResults => newSearch ? response.results : [...prevResults, ...response.results]);
      setTotalHits(response.pagination.total_count);
      if (!newSearch) {
        setOffset(prevOffset => prevOffset + response.results.length);
      } else {
        setOffset(response.results.length);
      }
    } catch (err) {
      console.error("Failed to search Modrinth projects:", err);
      setError(`${err.message}`);
      if (newSearch) {
        setSearchResults([]);
        setTotalHits(0);
        setOffset(0);
      }
    } finally {
      setLoading(false);
      
      // Set up delayed "No results found" message only for new searches
      if (newSearch) {
        noResultsTimeoutRef.current = setTimeout(() => {
          setShowNoResultsMessage(true);
        }, 250); // Show message after 1.5 seconds
      }
    }
  }, [
    searchTerm, projectType, offset, limit, sortOrder, modSource,
    currentSelectedCategories, selectedGameVersions, currentSelectedLoaders,
    filterClientRequired, filterServerRequired,
    allCategoriesData, allLoadersData, gameVersionsData,
    getCacheKey, searchCache
  ]);

  useEffect(() => {
    // Skip regular search if using VXL Studios data
    if (useVXLStudiosData) return;

    debugLog('[ModrinthSearchV2] useEffect for search triggered. Calling performSearch(true). Params:', {
      searchTerm,
      projectType,
      modSource,
      categories: currentSelectedCategories,
      gameVersions: selectedGameVersions,
      loaders: currentSelectedLoaders
    });

    // Scroll to top when filters/search term changes
    if (searchResultsAreaRef.current) {
      searchResultsAreaRef.current.scrollTop = 0;
    }

    // Reset expanded versions when filter changes
    setExpandedVersions({});
    setNumDisplayedVersions({});
    setVersionFilters({});

    performSearch(true);
  }, [
    searchTerm, projectType, sortOrder, modSource,
    currentSelectedCategories, selectedGameVersions, currentSelectedLoaders,
    filterClientRequired, filterServerRequired,
    useVXLStudiosData
  ]);

  const handleProjectTypeChange = (newProjectType: ModrinthProjectType) => {
    setProjectType(newProjectType);
  };

  // Simplified handleCategoryToggle - all category groups are multi-select
  const handleCategoryToggle = (categoryName: string) => {
    const currentSelectionsForActiveType = selectedCategoriesByProjectType[projectType] || [];
    const wasPreviouslySelected = currentSelectionsForActiveType.includes(categoryName);

    setSelectedCategoriesByProjectType(prevGlobalSelections => {
      const updatedSelectionsForCurrentType = wasPreviouslySelected
        ? currentSelectionsForActiveType.filter(c => c !== categoryName)
        : [...currentSelectionsForActiveType, categoryName];
      
      const newGlobalSelections = { ...prevGlobalSelections, [projectType]: updatedSelectionsForCurrentType };

      // Synchronize with other project types
      const effectiveAllowedTypes = allowedProjectTypes || ALL_MODRINTH_PROJECT_TYPES;
      for (const otherPT of effectiveAllowedTypes) {
        if (otherPT === projectType) continue; // Skip the currently active type

        const selectionsForOtherPT = newGlobalSelections[otherPT] || [];
        
        if (wasPreviouslySelected) {
          // Category was REMOVED from the active project type
          // So, remove it from other project types as well if it was selected there
          if (selectionsForOtherPT.includes(categoryName)) {
            newGlobalSelections[otherPT] = selectionsForOtherPT.filter(c => c !== categoryName);
          }
        } else {
          // Category was ADDED to the active project type
          // Add it to other project types if the category is defined for them and not already present
          const categoryDefinitionForOtherPT = allCategoriesData.find(
            catDef => catDef.name === categoryName && catDef.project_type === otherPT
          );
          if (categoryDefinitionForOtherPT) {
            if (!selectionsForOtherPT.includes(categoryName)) {
              newGlobalSelections[otherPT] = [...selectionsForOtherPT, categoryName];
            }
          }
        }
      }
      return newGlobalSelections;
    });
  };

  const handleGameVersionToggle = (version: string) => {
    setSelectedGameVersions(prev =>
      prev.includes(version)
        ? prev.filter(v => v !== version)
        : [...prev, version]
    );
  };

  const handleLoaderToggle = (loaderName: string) => {
    const currentSelectionsForActiveType = selectedLoadersByProjectType[projectType] || [];
    const wasPreviouslySelected = currentSelectionsForActiveType.includes(loaderName);

    setSelectedLoadersByProjectType(prevGlobalSelections => {
      const updatedSelectionsForCurrentType = wasPreviouslySelected
        ? currentSelectionsForActiveType.filter(l => l !== loaderName)
        : [...currentSelectionsForActiveType, loaderName];
      
      const newGlobalSelections = { ...prevGlobalSelections, [projectType]: updatedSelectionsForCurrentType };

      // Synchronize with other project types
      const effectiveAllowedTypes = allowedProjectTypes || ALL_MODRINTH_PROJECT_TYPES;
      for (const otherPT of effectiveAllowedTypes) {
        if (otherPT === projectType) continue; // Skip the currently active type

        const selectionsForOtherPT = newGlobalSelections[otherPT] || [];
        const loaderDefinition = allLoadersData.find(ldrDef => ldrDef.name === loaderName);

        if (wasPreviouslySelected) {
          // Loader was REMOVED from the active project type
          // So, remove it from other project types as well if it was selected there
          if (selectionsForOtherPT.includes(loaderName)) {
            newGlobalSelections[otherPT] = selectionsForOtherPT.filter(l => l !== loaderName);
          }
        } else {
          // Loader was ADDED to the active project type
          // Add it to other supported project types if not already present
          if (loaderDefinition && loaderDefinition.supported_project_types.includes(otherPT)) {
            if (!selectionsForOtherPT.includes(loaderName)) {
              newGlobalSelections[otherPT] = [...selectionsForOtherPT, loaderName];
            }
          }
        }
      }
      return newGlobalSelections;
    });
  };
  
  const loadMoreResults = () => {
    if (!loading && searchResults.length < totalHits) {
      performSearch(false);
    }
  };

  // Functions to remove individual filter tags
  const removeGameVersionTag = (version: string) => handleGameVersionToggle(version);
  const removeLoaderTag = (loaderName: string) => handleLoaderToggle(loaderName);
  const removeCategoryTag = (categoryName: string) => handleCategoryToggle(categoryName);
  const removeClientRequiredTag = () => setFilterClientRequired(false);
  const removeServerRequiredTag = () => setFilterServerRequired(false);

  const clearAllFilters = () => {
    setSelectedGameVersions([]);
    setSelectedCategoriesByProjectType(prev => ({ ...prev, [projectType]: [] }));
    setSelectedLoadersByProjectType(prev => ({ ...prev, [projectType]: [] }));
    setGameVersionSearchTerm(''); 
    setShowAllGameVersionsSidebar(false); // Reset new state to false
    setFilterClientRequired(false); // Reset new filter
    setFilterServerRequired(false); // Reset new filter
  };

  const toggleProjectVersions = async (projectId: string) => {
    if (expandedVersions[projectId] === 'loading') return;

    if (expandedVersions[projectId]) { 
      setExpandedVersions(prev => ({ ...prev, [projectId]: null }));
      // Reset the display count when versions are hidden
      setNumDisplayedVersions(prev => {
        const newState = { ...prev };
        delete newState[projectId];
        return newState;
      });
      // Clear version filters for this project
      setVersionFilters(prev => {
        const newState = { ...prev };
        delete newState[projectId];
        return newState;
      });
      // Clear version dropdown UI state for this project
      setVersionDropdownUIState(prev => {
        const newState = { ...prev };
        delete newState[projectId];
        return newState;
      });
    } else { 
      await loadProjectVersions(projectId);
    }
  };

  const loadProjectVersions = async (projectId: string) => {
    setExpandedVersions(prev => ({ ...prev, [projectId]: 'loading' }));
    try {
      debugLog(`Fetching versions for project: ${projectId}`);
      const response = await UnifiedService.getModVersions({
        source: modSource,
        project_id: projectId
      });
      
      const sortedVersions = response.versions.sort((a, b) => new Date(b.date_published).getTime() - new Date(a.date_published).getTime());

      setExpandedVersions(prev => ({ ...prev, [projectId]: sortedVersions }));
      // Initialize the number of displayed versions for this project
      setNumDisplayedVersions(prev => ({ ...prev, [projectId]: initialDisplayCount }));

      // Initialize version filters with main search selections
      setVersionFilters(prev => ({
        ...prev,
        [projectId]: {
          gameVersions: [...selectedGameVersions], // Start with main search selections
          loaders: [...currentSelectedLoaders],    // Start with main search selections
          versionType: 'all'  // Standardmäßig immer 'all' verwenden, nicht vom showReleaseGameVersionsOnly abhängig machen
        }
      }));

      // Initialize version dropdown UI state
      setVersionDropdownUIState(prev => ({
        ...prev,
        [projectId]: {
          showAllGameVersions: false, // Default to OFF
          gameVersionSearchTerm: '',
        }
      }));

      // No longer checking installation status for all versions here
    } catch (err) {
      console.error(`Failed to load versions for project ${projectId}:`, err);
      setExpandedVersions(prev => ({ ...prev, [projectId]: null }));
      setNumDisplayedVersions(prev => {
        const newState = { ...prev };
        delete newState[projectId];
        return newState;
      });
       // Clear version dropdown UI state on error too
      setVersionDropdownUIState(prev => {
        const newState = { ...prev };
        delete newState[projectId];
        return newState;
      });
    }
  };
  

  // Handler for version filter changes
  const handleVersionFilterChange = (projectId: string, filterType: 'gameVersions' | 'loaders' | 'versionType', value: string | string[]) => {
    setVersionFilters(prev => ({
      ...prev,
      [projectId]: {
        ...prev[projectId],
        [filterType]: value
      }
    }));
  };
  
  // Modified useEffect for version display - now checks status when versions are displayed
  useEffect(() => {
    // For each expanded project with a display count, check status of visible versions
    Object.entries(expandedVersions).forEach(([projectId, versions]) => {
      if (Array.isArray(versions) && versions.length > 0 && selectedProfile) {
        const displayCount = numDisplayedVersions[projectId] || initialDisplayCount;
        
        // Get filtered versions
        const filteredVersions = getFilteredVersions(projectId, versions);
        
        // Check status only for versions that will be displayed
        checkDisplayedVersionsStatusRef.current?.(
          projectId,
          filteredVersions,
          0,
          displayCount,
        );
      }
    });
  }, [expandedVersions, numDisplayedVersions, selectedProfile, versionFilters]);
  
  // Modify loadMoreProjectVersions to check installation status for newly displayed versions
  const loadMoreProjectVersions = (projectId: string) => {
    const currentDisplayCount = numDisplayedVersions[projectId] || INITIAL_DISPLAY_COUNT;
    const newDisplayCount = currentDisplayCount + LOAD_MORE_INCREMENT;

    setNumDisplayedVersions((prev) => ({
      ...prev,
      [projectId]: newDisplayCount,
    }));

    const versions = expandedVersions[projectId];
    const checkFn = checkDisplayedVersionsStatusRef.current;
    if (Array.isArray(versions) && selectedProfile && checkFn) {
      const filteredVersions = getFilteredVersions(projectId, versions);
      checkFn(
        projectId,
        filteredVersions,
        currentDisplayCount,
        LOAD_MORE_INCREMENT,
      );
    }
  };

  // Filter function for versions
  const getFilteredVersions = (projectId: string, versions: UnifiedVersion[]) => {
    if (!versionFilters[projectId]) return versions;
    
    const filters = versionFilters[projectId];
    
    return versions.filter(version => {
      // Filter by version type
      if (filters.versionType !== 'all' && version.release_type !== filters.versionType) {
        return false;
      }
      
      // Filter by game versions (if any selected)
      if (filters.gameVersions.length > 0) {
        const hasMatchingGameVersion = version.game_versions.some(gv => 
          filters.gameVersions.includes(gv)
        );
        if (!hasMatchingGameVersion) return false;
      }
      
      // Filter by loaders (if any selected)
      if (filters.loaders.length > 0) {
        const hasMatchingLoader = version.loaders.some(loader => 
          filters.loaders.includes(loader)
        );
        if (!hasMatchingLoader) return false;
      }
      
      return true;
    });
  };

  // Find the selected profile when the component mounts or selectedProfileId changes
  useEffect(() => {
    if (selectedProfileId && internalProfiles.length > 0) {
      const profile = internalProfiles.find(p => p.id === selectedProfileId);
      if (profile) {
        setSelectedProfile(profile);
      }
    } else if (selectedProfileId === '') {
      // Explicit empty selection - set to null
      setSelectedProfile(null);
    } else if (internalProfiles.length > 0 && !selectedProfile && selectedProfileId !== '' && selectedProfileId !== undefined) {
      // Auto-select first profile ONLY if:
      // - We have profiles
      // - No profile is currently selected
      // - No empty selection was requested (selectedProfileId !== '')
      // - selectedProfileId is not undefined (meaning it was explicitly passed as a prop)
      setSelectedProfile(internalProfiles[0]);
    }
  }, [selectedProfileId, internalProfiles, selectedProfile]);

  // Reset profile selection if explicit empty option was requested
  useEffect(() => {
    if (selectedProfileId === '') {
      setSelectedProfile(null);
      // Reset filters related to profile
      setSelectedGameVersions([]);
      setSelectedLoadersByProjectType(prev => ({
        ...prev,
        [projectType]: []
      }));
    }
  }, [selectedProfileId, projectType]);

  // Apply profile filters when selected profile changes - only set relevant filters based on project type
  useEffect(() => {
    if (selectedProfile) {
      // Set game version filter from profile - applicable to all project types
      if (selectedProfile.game_version) {
        setSelectedGameVersions([selectedProfile.game_version]);
      }
      
      // Set loader filter from profile - only for project types that use loaders
      if (selectedProfile.loader && ['mod', 'modpack'].includes(projectType)) {
        setSelectedLoadersByProjectType(prev => ({
          ...prev,
          [projectType]: [selectedProfile.loader]
        }));
      }
    }
  }, [selectedProfile, projectType]);

  const accentColor = useThemeStore((state) => state.accentColor); // Get accent color
  const displayContext = useDisplayContextStore((state) => state.context);
  const isStandaloneBrowse =
    overrideDisplayContext === "standalone" ||
    (overrideDisplayContext !== "detail" && displayContext !== "detail");
  const [hoveredVersionId, setHoveredVersionId] = useState<string | null>(null); // New state for version hover
  const [openVersionDropdowns, setOpenVersionDropdowns] = useState<Record<string, { type: boolean; gameVersion: boolean; loader: boolean }>>({});

  const toggleVersionDropdown = (projectId: string, dropdownType: 'type' | 'gameVersion' | 'loader') => {
    setOpenVersionDropdowns(prev => {
      const currentProjectDropdowns = prev[projectId] || { type: false, gameVersion: false, loader: false };
      const isOpen = currentProjectDropdowns[dropdownType];
      
      // Close all dropdowns for this project first, then open the target one if it was closed
      const newStateForProject = {
        type: false,
        gameVersion: false,
        loader: false,
        [dropdownType]: !isOpen, // Toggle the state of the clicked dropdown
      };

      return {
        ...prev,
        [projectId]: newStateForProject,
      };
    });
  };

  const closeAllVersionDropdowns = (projectId: string) => {
    setOpenVersionDropdowns(prev => ({
      ...prev,
      [projectId]: { type: false, gameVersion: false, loader: false },
    }));
  };

  // New state for version filtering UI controls within the expanded view
  const [versionDropdownUIState, setVersionDropdownUIState] = useState<Record<string, {
    showAllGameVersions: boolean;
    gameVersionSearchTerm: string;
  }>>({});

  // Handler for version dropdown UI state changes
  const handleVersionDropdownUIChange = (projectId: string, field: keyof typeof versionDropdownUIState[string], value: boolean | string) => {
    setVersionDropdownUIState(prev => ({
      ...prev,
      [projectId]: {
        ...prev[projectId],
        [field]: value,
      },
    }));
  };
  useEffect(() => {
    return () => {
      if (noResultsTimeoutRef.current) {
        clearTimeout(noResultsTimeoutRef.current);
      }
    };
  }, []);

  return {
    searchResultsAreaRef,
    searchTerm,
    setSearchTerm,
    projectType,
    searchResults,
    loading,
    error,
    totalHits,
    sortOrder,
    setSortOrder,
    sortOptions,
    allCategoriesData,
    gameVersionsData,
    allLoadersData,
    selectedCategoriesByProjectType,
    selectedLoadersByProjectType,
    setSelectedLoadersByProjectType,
    selectedGameVersions,
    setSelectedGameVersions,
    showAllGameVersionsSidebar,
    setShowAllGameVersionsSidebar,
    gameVersionSearchTerm,
    setGameVersionSearchTerm,
    filterClientRequired,
    setFilterClientRequired,
    filterServerRequired,
    setFilterServerRequired,
    expandedVersions,
    numDisplayedVersions,
    versionFilters,
    isSidebarVisible,
    setIsSidebarVisible,
    selectedProfile,
    setSelectedProfile,
    modSource,
    setModSource,
    internalProfiles,
    setInternalProfiles,
    currentSelectedCategories,
    currentSelectedLoaders,
    availableProjectTypes,
    availableLoaders,
    displayedGameVersions,
    dynamicFilterGroups,
    showNoResultsMessage,
    handleProjectTypeChange,
    handleCategoryToggle,
    handleGameVersionToggle,
    handleLoaderToggle,
    loadMoreResults,
    removeGameVersionTag,
    removeLoaderTag,
    removeCategoryTag,
    removeClientRequiredTag,
    removeServerRequiredTag,
    clearAllFilters,
    toggleProjectVersions,
    handleVersionFilterChange,
    loadMoreProjectVersions,
    getFilteredVersions,
    hoveredVersionId,
    setHoveredVersionId,
    openVersionDropdowns,
    toggleVersionDropdown,
    closeAllVersionDropdowns,
    versionDropdownUIState,
    handleVersionDropdownUIChange,
    accentColor,
    isStandaloneBrowse,
    overrideDisplayContext,
    initialDisplayCount: INITIAL_DISPLAY_COUNT,
  };
}

export type UseModrinthSearchReturn = ReturnType<typeof useModrinthSearch>;
