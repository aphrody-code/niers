"use client";

import * as React from "react";
import { format } from "date-fns";
import { fr } from "date-fns/locale";
import { Calendar as CalendarIcon } from "lucide-react";

import { cn } from "../../lib/utils";
import { Button } from "./button";
import { Calendar } from "./calendar";
import { Popover, PopoverContent, PopoverTrigger } from "./popover";

interface DatePickerProps {
	date?: Date;
	setDate: (date?: Date) => void;
	placeholder?: string;
	className?: string;
}

export function DatePicker({
	date,
	setDate,
	placeholder = "Choisir une date",
	className,
}: DatePickerProps) {
	return (
		<Popover>
			<PopoverTrigger
				render={
					<Button
						variant={"outline"}
						className={cn(
							"w-full justify-start text-left font-normal",
							!date && "text-muted-foreground",
							className
						)}
					>
						<CalendarIcon className="mr-2 h-4 w-4" />
						{date ? format(date, "PPP", { locale: fr }) : <span>{placeholder}</span>}
					</Button>
				}
			/>
			<PopoverContent className="w-auto p-0" align="start">
				<Calendar mode="single" selected={date} onSelect={setDate} autoFocus locale={fr} />
			</PopoverContent>
		</Popover>
	);
}
