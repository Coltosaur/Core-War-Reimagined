import { api } from './client';

export type LeaderboardEntry = {
  rank: number;
  user_id: string;
  username: string;
  rating: number;
  created_at: string;
};

export type LeaderboardResponse = {
  entries: LeaderboardEntry[];
  total: number;
  page: number;
  per_page: number;
};

export async function getLeaderboard(page = 1, perPage = 50): Promise<LeaderboardResponse> {
  return api.get<LeaderboardResponse>(`/api/leaderboard?page=${page}&per_page=${perPage}`);
}
