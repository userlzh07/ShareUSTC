import { apiClient } from './client';
import type { HelloResponse } from '../types';

export const helloApi = {
  async getHello(): Promise<HelloResponse> {
    const response = await apiClient.get<HelloResponse>('/api/hello');
    return response.data;
  },

  async healthCheck(): Promise<{ status: string; service: string }> {
    const response = await apiClient.get('/api/health');
    return response.data;
  }
};
