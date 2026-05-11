import { api } from './client';

export type ServerWarrior = {
  id: string;
  user_id: string;
  name: string;
  source: string;
  created_at: string;
  updated_at: string;
};

export type WarriorListResponse = {
  warriors: ServerWarrior[];
  total: number;
  page: number;
  per_page: number;
};

export async function listWarriors(page = 1, perPage = 100): Promise<WarriorListResponse> {
  return api.get<WarriorListResponse>(`/api/warriors?page=${page}&per_page=${perPage}`);
}

export async function getWarrior(id: string): Promise<ServerWarrior> {
  return api.get<ServerWarrior>(`/api/warriors/${id}`);
}

export async function createWarrior(name: string, source: string): Promise<ServerWarrior> {
  return api.post<ServerWarrior>('/api/warriors', { name, source });
}

export async function updateWarrior(
  id: string,
  patch: { name?: string; source?: string },
): Promise<ServerWarrior> {
  return api.put<ServerWarrior>(`/api/warriors/${id}`, patch);
}

export async function deleteWarrior(id: string): Promise<void> {
  return api.delete(`/api/warriors/${id}`);
}
