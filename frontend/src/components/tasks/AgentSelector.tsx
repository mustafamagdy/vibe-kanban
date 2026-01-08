import { Bot, ArrowDown } from 'lucide-react';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
import { Label } from '@/components/ui/label';
import type { ExecutorProfileId, BaseCodingAgent } from 'shared/types';

interface AgentSelectorProps {
  profiles: Record<string, Record<string, unknown>> | null;
  selectedExecutorProfile: ExecutorProfileId | null;
  onChange: (profile: ExecutorProfileId) => void;
  disabled?: boolean;
  className?: string;
  showLabel?: boolean;
}

interface FlattenedAgent {
  executor: BaseCodingAgent;
  variant: string | null;
  displayName: string;
}

function flattenAgents(profiles: Record<string, Record<string, unknown>>): FlattenedAgent[] {
  const result: FlattenedAgent[] = [];
  for (const [agent, variants] of Object.entries(profiles).sort()) {
    if (agent === 'CUSTOM') {
      // Flatten CUSTOM variants - skip DEFAULT, show named variants
      for (const [variantKey, config] of Object.entries(variants || {})) {
        if (variantKey === 'DEFAULT') continue; // Skip DEFAULT variant in UI
        // Config structure is { CUSTOM: { name?: string, ... } }
        const customConfig = (config as { CUSTOM?: { name?: string } })?.CUSTOM;
        const name = customConfig?.name || variantKey;
        result.push({
          executor: 'CUSTOM' as BaseCodingAgent,
          variant: variantKey,
          displayName: name,
        });
      }
    } else {
      // Built-in agents appear as-is
      result.push({
        executor: agent as BaseCodingAgent,
        variant: null,
        displayName: agent,
      });
    }
  }
  return result;
}

export function AgentSelector({
  profiles,
  selectedExecutorProfile,
  onChange,
  disabled,
  className = '',
  showLabel = false,
}: AgentSelectorProps) {
  const flattenedAgents = profiles ? flattenAgents(profiles) : [];

  // Build display name from selected profile
  const getDisplayName = (): string => {
    if (!selectedExecutorProfile) return 'Agent';

    const { executor, variant } = selectedExecutorProfile;
    if (executor === 'CUSTOM' && variant) {
      // Look up the name from profiles - config structure is { CUSTOM: { name?: string } }
      const customVariants = profiles?.CUSTOM;
      if (customVariants) {
        const config = customVariants[variant] as { CUSTOM?: { name?: string } } | undefined;
        return config?.CUSTOM?.name || variant;
      }
      return variant;
    }
    return executor;
  };

  if (!profiles) return null;

  return (
    <div className="flex-1">
      {showLabel && (
        <Label htmlFor="executor-profile" className="text-sm font-medium">
          Agent
        </Label>
      )}
      <DropdownMenu>
        <DropdownMenuTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            className={`w-full justify-between text-xs ${showLabel ? 'mt-1.5' : ''} ${className}`}
            disabled={disabled}
            aria-label="Select agent"
          >
            <div className="flex items-center gap-1.5 w-full">
              <Bot className="h-3 w-3" />
              <span className="truncate">{getDisplayName()}</span>
            </div>
            <ArrowDown className="h-3 w-3" />
          </Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-60">
          {flattenedAgents.length === 0 ? (
            <div className="p-2 text-sm text-muted-foreground text-center">
              No agents available
            </div>
          ) : (
            flattenedAgents.map(({ executor, variant, displayName }) => {
              const isSelected =
                selectedExecutorProfile?.executor === executor &&
                (executor !== 'CUSTOM' || selectedExecutorProfile?.variant === variant);

              return (
                <DropdownMenuItem
                  key={`${executor}-${variant}`}
                  onClick={() => {
                    onChange({
                      executor,
                      variant,
                    });
                  }}
                  className={isSelected ? 'bg-accent' : ''}
                >
                  {displayName}
                </DropdownMenuItem>
              );
            })
          )}
        </DropdownMenuContent>
      </DropdownMenu>
    </div>
  );
}
