import { Search } from "lucide-react";
import * as React from "react";
import { cn } from "../../lib/utils";

interface SearchBarProps extends React.ComponentProps<"div"> {
	placeholder?: string;
	defaultValue?: string;
	onSearch?: (value: string) => void;
}

const SearchBar = React.forwardRef<HTMLDivElement, SearchBarProps>(
	({ className, placeholder = "Rechercher...", defaultValue, onSearch, ...props }, ref) => {
		const timerRef = React.useRef<ReturnType<typeof setTimeout>>(null);

		const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
			const { value } = e.target;
			if (timerRef.current) {
				clearTimeout(timerRef.current);
			}
			timerRef.current = setTimeout(() => onSearch?.(value), 400);
		};

		const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
			if (e.key === "Enter") {
				if (timerRef.current) {
					clearTimeout(timerRef.current);
				}
				onSearch?.(e.currentTarget.value);
			}
		};

		return (
			<div
				ref={ref}
				className={cn(
					"flex h-14 w-full items-center gap-4 rounded-full bg-surface-container-high px-4 text-on-surface shadow-none transition-all focus-within:bg-surface-container-highest focus-within:shadow-level-1",
					className
				)}
				{...props}
			>
				<Search className="size-6 text-on-surface-variant" />
				<input
					type="text"
					placeholder={placeholder}
					defaultValue={defaultValue}
					className="flex-1 bg-transparent py-2 text-base outline-hidden placeholder:text-on-surface-variant"
					onChange={handleChange}
					onKeyDown={handleKeyDown}
				/>
			</div>
		);
	}
);

SearchBar.displayName = "SearchBar";

export { SearchBar };
