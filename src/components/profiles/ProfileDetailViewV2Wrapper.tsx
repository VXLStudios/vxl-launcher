"use client";

import { useEffect, useState } from "react";
import { useParams, useNavigate } from "react-router-dom";
import { useProfileStore } from "../../store/profile-store";
import { EmptyState, LoadingState } from "../ui-v2";
import { ProfileDetailViewV2 } from "./ProfileDetailViewV2";
import type { Profile } from "../../types/profile";
import { useProfileSettingsStore } from "../../store/profile-settings-store";

export function ProfileDetailViewV2Wrapper() {
  const { profileId } = useParams<{ profileId: string }>();
  const navigate = useNavigate();
  const { profiles, loading, fetchProfiles, profilesLoaded, getProfile } =
    useProfileStore();
  const [fetchedProfile, setFetchedProfile] = useState<Profile | null>(null);
  const [profileLoading, setProfileLoading] = useState(false);

  const profileFromList = profileId
    ? profiles.find((p) => p.id === profileId) ?? null
    : null;
  const profile = profileFromList ?? fetchedProfile;

  // Only fetch profiles once on initial mount if not already loaded
  useEffect(() => {
    if (!profilesLoaded && !loading) {
      void fetchProfiles();
    }
  }, [profilesLoaded, loading, fetchProfiles]);

  useEffect(() => {
    if (!profileId) {
      setFetchedProfile(null);
      setProfileLoading(false);
      return;
    }

    // Wait for the initial list fetch; do not abort lookup when a background refresh sets loading.
    if (!profilesLoaded) {
      return;
    }

    const foundProfile = profiles.find((p) => p.id === profileId);
    const listHasFullMods =
      foundProfile != null &&
      (foundProfile.mods.length > 0 ||
        (foundProfile.mod_count ?? 0) === 0);

    if (foundProfile && listHasFullMods) {
      setFetchedProfile(foundProfile);
      setProfileLoading(false);
      return;
    }

    let cancelled = false;
    setProfileLoading(true);

    void getProfile(profileId)
      .then((loadedProfile) => {
        if (!cancelled) {
          setFetchedProfile(loadedProfile);
        }
      })
      .catch(() => {
        if (!cancelled) {
          setFetchedProfile(null);
        }
      })
      .finally(() => {
        if (!cancelled) {
          setProfileLoading(false);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [profileId, profiles, profilesLoaded, getProfile]);

  const { openModal } = useProfileSettingsStore();

  const handleClose = () => {
    navigate("/profiles");
  };

  const handleEdit = () => {
    if (profile) {
      openModal(profile);
    }
  };

  if (!profileId) {
    return (
      <EmptyState
        icon="solar:danger-triangle-bold"
        title="No profile ID provided"
        description="Return to the profiles list and try again."
      />
    );
  }

  if (profile) {
    return (
      <ProfileDetailViewV2
        profile={profile}
        onClose={handleClose}
        onEdit={handleEdit}
      />
    );
  }

  if (!profilesLoaded || profileLoading || loading) {
    return <LoadingState message="Loading profile..." />;
  }

  return (
    <EmptyState
      icon="solar:widget-bold"
      title="Profile not found"
      description="This profile may have been deleted or moved."
    />
  );
}
