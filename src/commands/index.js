// Registers every command with the registry (import for side effects).
import './draw.js';
import './modify.js';
import './annotate.js';
import './inquiry.js';
import './layer.js';
import './view.js';
import './blocks.js';
import './solids.js';
import './system.js';
import { installAliases } from './aliases.js';

installAliases();
