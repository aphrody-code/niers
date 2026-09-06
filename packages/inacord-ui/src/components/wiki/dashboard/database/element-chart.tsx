"use client";

import type * as React from "react";
import { Cell, PieChart, Pie as RcPie, Tooltip as RcTooltip, ResponsiveContainer } from "recharts";
import { Card, CardContent, CardHeader, CardTitle } from "../../../../components/ui/card";

// Cast to FC<any> to absorb React 19 / recharts 2.x typings drift (TS2607/TS2786).
// Remove once recharts ships React 19-compatible types.
const Pie = RcPie as unknown as React.FC<any>;
const Tooltip = RcTooltip as unknown as React.FC<any>;

const COLORS = {
	Earth: "#FF9800", // Orange
	Fire: "#F44336", // Red
	Void: "#9C27B0", // Purple
	Wind: "#00BFA5", // Teal
	Wood: "#4CAF50", // Green
};

export function ElementChart({ data }: { data: Array<{ name: string; value: number }> }) {
	return (
		<Card className="col-span-2 rounded-[32px] border-none shadow-sm bg-surface-container-low">
			<CardHeader>
				<CardTitle className="text-lg font-bold uppercase tracking-wider text-on-surface">
					Distribution Élémentaire
				</CardTitle>
			</CardHeader>
			<CardContent>
				<div className="h-[300px] w-full">
					<ResponsiveContainer width="100%" height="100%">
						<PieChart>
							<Pie
								data={data}
								cx="50%"
								cy="50%"
								innerRadius={60}
								outerRadius={100}
								paddingAngle={5}
								dataKey="value"
							>
								{data.map((entry, index) => (
									<Cell
										key={`cell-${index}`}
										fill={(COLORS as any)[entry.name] || "#8884d8"}
										strokeWidth={0}
									/>
								))}
							</Pie>
							<Tooltip
								contentStyle={{
									border: "none",
									borderRadius: "12px",
									boxShadow: "0 4px 12px rgba(0,0,0,0.1)",
								}}
								itemStyle={{ color: "#000", fontWeight: "bold" }}
							/>
						</PieChart>
					</ResponsiveContainer>
				</div>
				<div className="flex flex-wrap justify-center gap-4 mt-4">
					{data.map((entry) => (
						<div key={entry.name} className="flex items-center gap-2">
							<div
								className="size-3 rounded-full"
								style={{ backgroundColor: (COLORS as any)[entry.name] || "#8884d8" }}
							/>
							<span className="text-sm font-medium text-on-surface-variant">
								{entry.name} ({entry.value})
							</span>
						</div>
					))}
				</div>
			</CardContent>
		</Card>
	);
}
