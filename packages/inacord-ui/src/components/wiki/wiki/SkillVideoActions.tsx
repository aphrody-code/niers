"use client";

import { Download, Loader2, Share2 } from "lucide-react";
import { useState } from "react";

interface SkillVideoActionsProps {
	videoUrl?: string;
	skillName: string;
	/** Libellé de la variante téléchargée (« Réussite », « Échec »…), s'il y en a plusieurs. */
	variantLabel?: string;
}

export function SkillVideoActions({ videoUrl, skillName, variantLabel }: SkillVideoActionsProps) {
	const [isDownloading, setIsDownloading] = useState(false);
	const [shareConfirmed, setShareConfirmed] = useState(false);

	const handleShare = async () => {
		if (typeof navigator !== "undefined" && navigator.share) {
			try {
				await navigator.share({
					text: `Découvre la technique ${skillName} sur Azalée !`,
					title: skillName,
					url: window.location.href,
				});
			} catch (error) {
				console.error("Error sharing:", error);
			}
		} else if (typeof navigator !== "undefined" && navigator.clipboard) {
			navigator.clipboard.writeText(window.location.href);
			setShareConfirmed(true);
			setTimeout(() => {
				setShareConfirmed(false);
			}, 2000);
		}
	};

	const handleDownload = async (e: React.MouseEvent) => {
		e.preventDefault();
		if (!videoUrl) {
			return;
		}

		setIsDownloading(true);
		try {
			// Try to fetch as blob to force download
			const response = await fetch(videoUrl);
			if (!response.ok) {
				throw new Error("Network response was not ok");
			}
			const blob = await response.blob();
			const blobUrl = window.URL.createObjectURL(blob);

			// Extension réelle du fichier servi : zukan publie du `.webm`, pas du mp4.
			const ext = videoUrl.split(".").pop() || "webm";

			const base = [skillName, variantLabel].filter(Boolean).join(" ");
			const link = document.createElement("a");
			link.href = blobUrl;
			link.download = `${base.replaceAll(/\s+/g, "_")}.${ext}`;
			document.body.append(link);
			link.click();
			document.body.removeChild(link);
			window.URL.revokeObjectURL(blobUrl);
		} catch (error) {
			console.warn("Direct download failed (likely CORS), falling back to new tab", error);
			window.open(videoUrl, "_blank");
		} finally {
			setIsDownloading(false);
		}
	};

	return (
		<>
			{videoUrl && (
				<a
					href={videoUrl}
					onClick={handleDownload}
					target="_blank"
					rel="noopener noreferrer"
					className={`inline-flex items-center gap-2 px-4 py-2 rounded-full text-sm font-medium transition-colors cursor-pointer min-h-11 sm:min-h-0 ${
						isDownloading
							? "bg-surface-container-high text-on-surface/50 cursor-wait"
							: "bg-surface-container-highest hover:bg-surface-container-high text-on-surface"
					}`}
				>
					{isDownloading ? (
						<Loader2 size={20} className="animate-spin" aria-hidden="true" />
					) : (
						<Download size={20} aria-hidden="true" />
					)}
					{isDownloading ? "..." : "Télécharger"}
				</a>
			)}

			<button
				onClick={handleShare}
				className="inline-flex items-center justify-center size-11 sm:size-9 rounded-full bg-surface-container-highest hover:bg-surface-container-high text-on-surface transition-colors"
				title="Partager"
			>
				{shareConfirmed ? (
					<span className="text-[20px]">&#10003;</span>
				) : (
					<Share2 size={20} aria-hidden="true" />
				)}
			</button>
		</>
	);
}
