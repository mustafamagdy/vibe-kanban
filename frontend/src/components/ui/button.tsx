import * as React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '@/lib/utils';

const buttonVariants = cva(
  'inline-flex items-center justify-center whitespace-nowrap text-sm font-medium ring-offset-background transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 disabled:cursor-not-allowed rounded-md border',
  {
    variants: {
      variant: {
        default:
          'bg-primary text-primary-foreground',
        destructive:
          'bg-destructive text-destructive-foreground',
        outline:
          'bg-muted text-foreground',
        secondary:
          'bg-secondary text-secondary-foreground',
        ghost:
          'bg-transparent text-foreground hover:bg-accent border-transparent',
        link: 'text-primary underline-offset-4 hover:underline border-transparent',
        icon: 'bg-transparent text-foreground border-transparent',
      },
      size: {
        default: 'h-10 px-4 py-2',
        xs: 'h-8 px-3 text-xs',
        sm: 'h-9 px-4',
        lg: 'h-12 px-6 text-base',
        icon: 'h-10 w-10',
      },
    },
    compoundVariants: [
      { variant: 'icon', size: 'icon', class: 'p-0 h-4' }
    ],
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  }
);

// Neobrutal button variants
const buttonVariantsNeobrutal = cva(
  'inline-flex items-center justify-center whitespace-nowrap text-sm font-medium ring-offset-background transition-all duration-150 focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring focus-visible:ring-offset-2 disabled:pointer-events-none disabled:opacity-50 disabled:cursor-not-allowed',
  {
    variants: {
      variant: {
        default:
          'neobrutal:bg-primary neobrutal:text-primary-foreground neobrutal:border-3 neobrutal:border-foreground',
        destructive:
          'neobrutal:bg-destructive neobrutal:text-destructive-foreground neobrutal:border-3 neobrutal:border-foreground',
        outline:
          'neobrutal:bg-muted neobrutal:text-foreground neobrutal:border-3 neobrutal:border-foreground',
        secondary:
          'neobrutal:bg-secondary neobrutal:text-secondary-foreground neobrutal:border-3 neobrutal:border-foreground',
        ghost:
          'neobrutal:bg-transparent neobrutal:text-foreground neobrutal:border-3 neobrutal:border-transparent neobrutal:hover:bg-accent',
        link: 'neobrutal:text-primary neobrutal:underline-offset-4 neobrutal:hover:underline neobrutal:border-transparent',
        icon: 'neobrutal:bg-transparent neobrutal:text-foreground neobrutal:border-3 neobrutal:border-transparent',
      },
      size: {
        default: 'h-10 px-4 py-2',
        xs: 'h-8 px-3 text-xs',
        sm: 'h-9 px-4',
        lg: 'h-12 px-6 text-base',
        icon: 'h-10 w-10',
      },
    },
    compoundVariants: [
      { variant: 'icon', size: 'icon', class: 'neobrutal:p-0 neobrutal:h-4' }
    ],
    defaultVariants: {
      variant: 'default',
      size: 'default',
    },
  }
);

export interface ButtonProps
  extends React.ButtonHTMLAttributes<HTMLButtonElement>,
    VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

const Button = React.forwardRef<HTMLButtonElement, ButtonProps>(
  ({ className, variant, size, asChild = false, ...props }, ref) => {
    const Comp = asChild ? Slot : 'button';
    return (
      <Comp
        className={cn(
          buttonVariants({ variant, size, className }),
          buttonVariantsNeobrutal({ variant, size, className })
        )}
        ref={ref}
        {...props}
      />
    );
  }
);
Button.displayName = 'Button';

export { Button, buttonVariants };
