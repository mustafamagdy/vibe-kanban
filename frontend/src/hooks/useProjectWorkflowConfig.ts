import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { projectsApi } from '@/lib/api';
import type { UpdateWorkflowConfigRequest, WorkflowConfigResponse } from 'shared/types';

type Options = {
  enabled?: boolean;
};

export function useProjectWorkflowConfig(projectId?: string, opts?: Options) {
  const enabled = (opts?.enabled ?? true) && !!projectId;

  return useQuery<WorkflowConfigResponse>({
    queryKey: ['projectWorkflowConfig', projectId],
    queryFn: () => projectsApi.getWorkflowConfig(projectId!),
    enabled,
  });
}

export function useUpdateProjectWorkflowConfig(projectId?: string) {
  const queryClient = useQueryClient();

  return useMutation({
    mutationFn: (data: UpdateWorkflowConfigRequest) =>
      projectsApi.updateWorkflowConfig(projectId!, data),
    onSuccess: (newConfig) => {
      // Invalidate and refetch the workflow config
      queryClient.invalidateQueries({ queryKey: ['projectWorkflowConfig', projectId] });
      // Update the cached data directly for optimistic updates
      queryClient.setQueryData<WorkflowConfigResponse>(
        ['projectWorkflowConfig', projectId],
        newConfig
      );
    },
  });
}
