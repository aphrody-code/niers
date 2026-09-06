"use client";

import * as React from "react";
import {
	Dialog,
	DialogContent,
	DialogDescription,
	DialogFooter,
	DialogHeader,
	DialogTitle,
} from "./dialog";
import {
	Drawer,
	DrawerContent,
	DrawerDescription,
	DrawerFooter,
	DrawerHeader,
	DrawerTitle,
} from "./drawer";
import { useMediaQuery } from "../../lib/use-media-query";
import { cn } from "../../lib/utils";

interface ResponsiveDialogProps {
	open: boolean;
	onOpenChange: (open: boolean) => void;
	title: React.ReactNode;
	description?: React.ReactNode;
	children: React.ReactNode;
	footer?: React.ReactNode;
	contentClassName?: string;
}

export function ResponsiveDialog({
	open,
	onOpenChange,
	title,
	description,
	children,
	footer,
	contentClassName,
}: ResponsiveDialogProps) {
	const [mounted, setMounted] = React.useState(false);
	const isDesktop = useMediaQuery("(min-width: 768px)");

	React.useEffect(() => {
		setMounted(true);
	}, []);

	if (!mounted) {
		return null;
	}

	if (isDesktop) {
		return (
			<Dialog open={open} onOpenChange={onOpenChange}>
				<DialogContent
					data-slot="responsive-dialog-content"
					className={cn(
						"max-w-2xl max-h-[90vh] overflow-y-auto sm:max-w-[600px]",
						contentClassName
					)}
				>
					<DialogHeader>
						<DialogTitle>{title}</DialogTitle>
						{description && <DialogDescription>{description}</DialogDescription>}
					</DialogHeader>
					{children}
					{footer && <DialogFooter>{footer}</DialogFooter>}
				</DialogContent>
			</Dialog>
		);
	}

	return (
		<Drawer open={open} onOpenChange={onOpenChange}>
			<DrawerContent data-slot="responsive-dialog-drawer" className="max-h-[90vh]">
				<DrawerHeader className="text-left">
					<DrawerTitle>{title}</DrawerTitle>
					{description && <DrawerDescription>{description}</DrawerDescription>}
				</DrawerHeader>
				<div className="px-4 overflow-y-auto">{children}</div>
				{footer ? <DrawerFooter>{footer}</DrawerFooter> : <DrawerFooter className="pt-2" />}
			</DrawerContent>
		</Drawer>
	);
}
