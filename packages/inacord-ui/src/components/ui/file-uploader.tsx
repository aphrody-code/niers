"use client";

import * as React from "react";
import { UploadCloud, X } from "lucide-react";
import { cn } from "../../lib/utils";

interface FileUploaderProps extends React.InputHTMLAttributes<HTMLInputElement> {
	onFilesSelected: (files: File[]) => void;
	maxFiles?: number;
	maxSize?: number; // en bytes
	accept?: string;
	className?: string;
}

export function FileUploader({
	onFilesSelected,
	maxFiles = 1,
	maxSize = 5 * 1024 * 1024, // 5MB
	accept = "image/*",
	className,
	...props
}: FileUploaderProps) {
	const [dragActive, setDragActive] = React.useState(false);
	const [files, setFiles] = React.useState<File[]>([]);
	const inputRef = React.useRef<HTMLInputElement>(null);

	const handleFiles = (newFiles: FileList | null) => {
		if (!newFiles) return;
		const validFiles = Array.from(newFiles).filter((file) => {
			return file.size <= maxSize && (!accept || file.type.match(accept.replace("*", ".*")));
		});

		const updatedFiles = [...files, ...validFiles].slice(0, maxFiles);
		setFiles(updatedFiles);
		onFilesSelected(updatedFiles);
	};

	return (
		<div className={cn("w-full", className)}>
			<div
				className={cn(
					"border-2 border-dashed rounded-lg p-6 flex flex-col items-center justify-center text-center cursor-pointer transition-colors",
					dragActive ? "border-primary bg-primary/5" : "border-muted-foreground/25",
					"hover:bg-accent/50"
				)}
				onDragOver={(e) => {
					e.preventDefault();
					setDragActive(true);
				}}
				onDragLeave={() => setDragActive(false)}
				onDrop={(e) => {
					e.preventDefault();
					setDragActive(false);
					handleFiles(e.dataTransfer.files);
				}}
				onClick={() => inputRef.current?.click()}
			>
				<UploadCloud className="h-10 w-10 text-muted-foreground mb-4" />
				<p className="text-sm text-muted-foreground mb-1">
					Glissez-déposez ou cliquez pour sélectionner
				</p>
				<p className="text-xs text-muted-foreground/70">
					Max {maxSize / 1024 / 1024}MB. {maxFiles > 1 ? `Jusqu'à ${maxFiles} fichiers.` : ""}
				</p>
				<input
					ref={inputRef}
					type="file"
					accept={accept}
					multiple={maxFiles > 1}
					className="hidden"
					onChange={(e) => handleFiles(e.target.files)}
					{...props}
				/>
			</div>

			{files.length > 0 && (
				<div className="mt-4 space-y-2">
					{files.map((file, idx) => (
						<div
							key={idx}
							className="flex items-center justify-between p-2 text-sm border rounded-md"
						>
							<span className="truncate max-w-[80%]">{file.name}</span>
							<button
								type="button"
								className="text-muted-foreground hover:text-destructive"
								onClick={() => {
									const updated = files.filter((_, i) => i !== idx);
									setFiles(updated);
									onFilesSelected(updated);
								}}
							>
								<X className="h-4 w-4" />
							</button>
						</div>
					))}
				</div>
			)}
		</div>
	);
}
