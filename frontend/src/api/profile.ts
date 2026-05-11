import { api } from './client';

export type ProfileStats = {
  user_id: string;
  username: string;
  created_at: string;
  warrior_count: number;
  match_count: number;
  wins: number;
  losses: number;
  ties: number;
};

export type PublicWarrior = {
  id: string;
  name: string;
  created_at: string;
  updated_at: string;
};

export type PublicProfile = ProfileStats & {
  warriors: PublicWarrior[];
};

export async function getMyProfile(): Promise<ProfileStats> {
  return api.get<ProfileStats>('/api/profile');
}

export async function getPublicProfile(username: string): Promise<PublicProfile> {
  return api.get<PublicProfile>(`/api/users/${encodeURIComponent(username)}`);
}
