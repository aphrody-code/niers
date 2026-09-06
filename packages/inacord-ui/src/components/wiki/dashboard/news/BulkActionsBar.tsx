"use client";

import { Archive, Check, FileText, Pin, PinOff, Trash2, X } from "lucide-react";
import { useState } from "react";
import { toast } from "sonner";
import { AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle } from "../../../../components/ui/alert-dialog";
import { Button } from "../../../../components/ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "../../../../components/ui/select";

interface BulkActionsBarProps {
	selectedCount: number;
	onClearSelection: () => void;
	onBulkAction: (action: string, value?: string) => Promise<void>;
}

export function BulkActionsBar({
	selectedCount,
	onClearSelection,
	onBulkAction,
}: BulkActionsBarProps) {
	const [loading, setLoading] = useState(false);
	const [deleteOpen, setDeleteOpen] = useState(false);

	const handleAction = async (action: string, value?: string) => {
		setLoading(true);
		try {
			await onBulkAction(action, value);
		} catch {
			toast.error("Erreur lors de l'action en masse");
		} finally {
			setLoading(false);
		}
	};

	if (selectedCount === 0) {
		return null;
	}

	return (
		<>
			<div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50 animate-in slide-in-from-bottom-4 fade-in duration-300">
				<div className="flex items-center gap-2 sm:gap-3 bg-inverse-surface text-inverse-on-surface px-4 sm:px-6 py-3 rounded-full shadow-lg">
					<span className="text-sm font-medium whitespace-nowrap">
						{selectedCount} sélectionné{selectedCount > 1 ? "s" : ""}
					</span>

					<div className="h-5 w-px bg-inverse-on-surface/20" />

					{/* Status change */}
					<Select
						onValueChange={(val) => handleAction("status", typeof val === "string" ? val : undefined)}
						disabled={loading}
					>
						<SelectTrigger className="h-8 w-auto gap-1.5 border-none bg-inverse-on-surface/10 text-inverse-on-surface text-xs rounded-full px-3">
							<SelectValue placeholder="Statut" />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="draft">
								<div className="flex items-center gap-2">
									<FileText className="size-3.5" />
									Brouillon
								</div>
							</SelectItem>
							<SelectItem value="published">
								<div className="flex items-center gap-2">
									<Check className="size-3.5" />
									Publié
								</div>
							</SelectItem>
							<SelectItem value="archived">
								<div className="flex items-center gap-2">
									<Archive className="size-3.5" />
									Archivé
								</div>
							</SelectItem>
						</SelectContent>
					</Select>

					{/* Category change */}
					<Select
						onValueChange={(val) => handleAction("category", typeof val === "string" ? val : undefined)}
						disabled={loading}
					>
						<SelectTrigger className="h-8 w-auto gap-1.5 border-none bg-inverse-on-surface/10 text-inverse-on-surface text-xs rounded-full px-3">
							<SelectValue placeholder="Catégorie" />
						</SelectTrigger>
						<SelectContent>
							<SelectItem value="announcement">Annonce</SelectItem>
							<SelectItem value="event">Événement</SelectItem>
							<SelectItem value="critique">Critique</SelectItem>
							<SelectItem value="community">Communauté</SelectItem>
						</SelectContent>
					</Select>

					{/* Pin / Unpin */}
					<Button
						variant="ghost"
						size="sm"
						disabled={loading}
						onClick={() => handleAction("pin", "true")}
						className="h-8 text-xs text-inverse-on-surface hover:bg-inverse-on-surface/10 rounded-full px-3"
						title="Épingler"
					>
						<Pin className="size-3.5 mr-1.5" />
						<span className="hidden sm:inline">Épingler</span>
					</Button>
					<Button
						variant="ghost"
						size="sm"
						disabled={loading}
						onClick={() => handleAction("pin", "false")}
						className="h-8 text-xs text-inverse-on-surface hover:bg-inverse-on-surface/10 rounded-full px-3"
						title="Désépingler"
					>
						<PinOff className="size-3.5 mr-1.5" />
						<span className="hidden sm:inline">Désépingler</span>
					</Button>

					<div className="h-5 w-px bg-inverse-on-surface/20" />

					{/* Delete */}
					<Button
						variant="ghost"
						size="sm"
						disabled={loading}
						onClick={() => setDeleteOpen(true)}
						className="h-8 text-xs text-red-300 hover:text-red-100 hover:bg-red-500/20 rounded-full px-3"
					>
						<Trash2 className="size-3.5 mr-1.5" />
						Supprimer
					</Button>

					<div className="h-5 w-px bg-inverse-on-surface/20" />

					{/* Close */}
					<Button
						variant="ghost"
						size="icon"
						onClick={onClearSelection}
						className="size-8 text-inverse-on-surface hover:bg-inverse-on-surface/10 rounded-full"
					>
						<X className="size-4" />
					</Button>
				</div>
			</div>

			{/* Delete confirmation dialog */}
			<AlertDialog open={deleteOpen} onOpenChange={setDeleteOpen}>
				<AlertDialogContent className="rounded-[28px] bg-surface border-outline-variant/20">
					<AlertDialogHeader>
						<AlertDialogTitle className="text-on-surface">
							Supprimer {selectedCount} article{selectedCount > 1 ? "s" : ""} ?
						</AlertDialogTitle>
						<AlertDialogDescription className="text-on-surface-variant">
							Cette action est irréversible. Les articles sélectionnés seront définitivement
							supprimés.
						</AlertDialogDescription>
					</AlertDialogHeader>
					<AlertDialogFooter>
						<AlertDialogCancel className="rounded-full">Annuler</AlertDialogCancel>
						<AlertDialogAction
							onClick={() => handleAction("delete")}
							className="rounded-full bg-red-600 hover:bg-red-700 text-white"
						>
							<Trash2 className="size-4 mr-2" />
							Supprimer
						</AlertDialogAction>
					</AlertDialogFooter>
				</AlertDialogContent>
			</AlertDialog>
		</>
	);
}
