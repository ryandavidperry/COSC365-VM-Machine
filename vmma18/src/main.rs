use std::io::{self, Read, Write};
use std::env::args;
use std::fs;

fn main() {
    // Check arguments
    let args: Vec<String> = args().collect();
    if args.len() != 2 {
        println!("Usage: {} <file.v>", &args[0]);
        return;
    }
    let filename = &args[1];

    let binary = fs::read(filename).expect("No such file or directory");

    // Convert the binary data into a vector of u32 instructions
    let program: Vec<u32> = binary
        .chunks(4)                                      
        .map(|chunk| u32::from_le_bytes(chunk.try_into().unwrap()))  
        .collect();

    // Create a new virtual machine instance
    let mut machine = Machine {
        ram: [0; 1024],     
        sp: 1024,            
        pc: 0,               
        input: io::stdin(),  
        output: io::stdout() 
    };

    // Load the program into the VM's memory
    machine.load(&program).unwrap();

    // Run the program and exit with its return code
    let exit_code = machine.run().unwrap();
    std::process::exit(exit_code.into());
}

// Virtual Machine structure, parameterized over input/output types (for testing flexibility)
struct Machine<R: Read, W: Write> {
    ram: [u32; 1024], 
    sp: i16,         
    pc: i16,         
    input: R,         
    output: W,        
}

// Instruction set (interpreted from RAM contents)
#[derive(Debug)]
enum Instruction {
    // Miscellaneous
    Exit(u8),        
    Swap(i16, i16),   
    Nop(),           
    Input(),          
    Stinput(u32),    
    Debug(u32),       

    Pop(u32),         

    // Binary Arithmetic
    Add(),
    Subtract(),
    Multiply(),
    Divide(),
    Remainder(),
    And(),
    Or(),
    Xor(),
    LogicalLeftShift(),
    LogicalRightShift(),
    ArithmeticRightShift(),

    // Unary Arithmetic
    Negate(),
    Not(),

    Stprint(i32),     

    Goto(i32),

    Call(i32),
    Return(u32),

    BinaryIf(u8, i32),

    // Unary If
    EqZero(i32),      
    NeZero(i32),      
    LtZero(i32),      
    GeZero(i32),     
    Dup(u32),
    Dump(),
    Print(i32),

    Push(u32),        
}

// Top-level instruction class based on opcode nibble
#[derive(Debug)]
enum Opcode {
    Miscellaneous, 
    BinaryArithmetic,
    UnaryArithmetic,
    Pop,          
    StringPrint,   
    Goto,
    Call,
    Return,
    UnaryIf,     
    BinaryIf,
    Dup,
    Print,
    Dump,
    Push,          
    Unknown,       
}

impl Opcode {

    // Convert the top 4 bits (opcode) of an instruction to an `Opcode` enum
    fn from_integer(n: u8) -> Opcode {
        match n {
            0x0 => Opcode::Miscellaneous,
            0x1 => Opcode::Pop,
            0x2 => Opcode::BinaryArithmetic,
            0x3 => Opcode::UnaryArithmetic,
            0x4 => Opcode::StringPrint,
            0x5 => Opcode::Call,
            0x6 => Opcode::Return,
            0x7 => Opcode::Goto,
            0x8 => Opcode::BinaryIf,
            0x9 => Opcode::UnaryIf,
            0xC => Opcode::Dup,
            0xD => Opcode::Print,
            0xE => Opcode::Dump,
            0xF => Opcode::Push,
            _ => Opcode::Unknown,
        }
    }
}

impl<R: Read, W: Write> Machine<R, W> {

    // Load a program into RAM, checking for magic number
    pub fn load(&mut self, program: &[u32]) -> Result<(), &'static str> {
        if program.first() != Some(&0xEFBE_ADDE) {
            return Err("Bad number"); 
        }

        // Load the program (skipping the magic word) into RAM
        self.ram[..program.len() - 1].copy_from_slice(&program[1..]);
        self.sp = 1024;
        self.pc = 0;

        Ok(())
    }

    // Run the virtual machine loop
    pub fn run(&mut self) -> Result<u8, Box<dyn std::error::Error>> {
        loop {
            let instruction = self.fetch(); 

            match instruction {
                Instruction::Exit(code) => return Ok(code),

                Instruction::Swap(from, to) => {
                    // Sign-extend the 12-bit offsets
                    let from_offset = (((from as i16) << 4) >> 2) as i16;
                    let to_offset = (((to as i16) << 4) >> 2) as i16;

                    // Swap two words in the stack (from and to are relative to SP)
                    let f = (self.sp + (from_offset >> 2)) as usize;
                    let t = (self.sp + (to_offset >> 2)) as usize;
                    self.ram.swap(f, t);
                }

                Instruction::Nop() => (), 

                Instruction::Input() => {
                    // Read a number (decimal/hex/bin) from user
                    let line = self.read_line()?;
                    let trimmed = line.trim();

                    let word = if let Some(stripped) = trimmed.strip_prefix("0x") {
                        i32::from_str_radix(stripped, 16)
                            .map_err(|_| "(input) hex input cannot be converted to an integer")
                    } else if let Some(stripped) = trimmed.strip_prefix("0b") {
                        i32::from_str_radix(stripped, 2)
                            .map_err(|_| "(input) binary input cannotbe converted to an integer")
                    } else {
                        i32::from_str_radix(trimmed, 10)
                            .map_err(|_| "(input) decimal input cannot be converted to an integer")
                    }?;

                    self.push(word as u32)?;
                }

                Instruction::Stinput(max_chars) => {

                    // Read a string from input and store it in RAM using 24-bit packing
                    let mut input = self.read_line()?.trim().to_string();
                    if input.is_empty() {
                        self.push(0)?;
                        continue;
                    }

                    input.truncate(max_chars as usize);

                    // Pad to 3-byte alignment with sentinel value 0x01
                    while input.len() % 3 != 0 {
                        input.push(0x01 as char);
                    }

                    // Encode string backwards into stack
                    for (i, chunk) in input.as_bytes().rchunks(3).enumerate() {
                        let word = ((chunk[2] as u32) << 16)
                            | ((chunk[1] as u32) << 8)
                            | (chunk[0] as u32)
                            | if i != 0 { 0x0100_0000 } else { 0 };

                        self.push(word)?;
                    }
                }

                Instruction::Debug(_offset) => {
                    println!("Debug");
                }

                /*
                 * Binary Arithmetic Instructions
                 */
                Instruction::Add()                  => self.binary_op(|l, r| l + r),
                Instruction::Subtract()             => self.binary_op(|l, r| l - r),
                Instruction::Multiply()             => self.binary_op(|l, r| l * r),
                Instruction::Divide()               => self.binary_op(|l, r| l / r),
                Instruction::Remainder()            => self.binary_op(|l, r| l % r),
                Instruction::And()                  => self.binary_op(|l, r| l & r),
                Instruction::Or()                   => self.binary_op(|l, r| l | r),
                Instruction::Xor()                  => self.binary_op(|l, r| l ^ r),
                Instruction::LogicalLeftShift()     => self.binary_op(|l, r| l << r),
                Instruction::LogicalRightShift()    => self.binary_op(|l, r| l >> r),
                Instruction::ArithmeticRightShift() => self.binary_op(|l, r| l as i32 >> r),

                /*
                 * Unary Arithmetic Instructions
                 */
                Instruction::Not() => self.unary_op(|x| !x),
                Instruction::Negate() => self.unary_op(|x| x.wrapping_neg()),

                Instruction::Pop(offset) => {

                    // Pop offset bytes (in 4-byte words) from the stack
                    self.sp = (self.sp + (offset >> 2) as i16).clamp(0, 1024);
                }

                Instruction::Goto(offset) => {
                    self.pc += offset as i16;
                    continue;
                }

                Instruction::Stprint(offset) => {

                    // Print a packed string from RAM starting at offset
                    let mut idx = (self.sp + (offset >> 2) as i16) as usize;
                    loop {
                        let cur_word = self.ram[idx];

                        if cur_word == 0 {
                            break;
                        }

                        let bytes = cur_word.to_le_bytes(); 
                        for &b in &bytes {
                            if b != 1 {
                                self.output.write_all(&[b])?;
                            }
                        }
                        if bytes[3] == 0 || idx == 0 {
                            break;
                        }
                        idx += 1;
                    }

                    self.output.flush()?;
                }


                Instruction::Call(offset) => {
                    // Push return address (next PC)
                    let return_address = (self.pc + 1) as u32;
                    self.push(return_address)?;

                    // Jump to offset
                    self.pc += (offset >> 2) as i16;
                    continue;
                },
                Instruction::Return(offset) => {
                    // Pop address from stack
                    let addr = self.ram.get(self.sp as usize).copied().unwrap_or(0);
                    self.sp += 1 + ((offset >> 2) as i16).clamp(0, 1024 - self.sp);
                    self.pc = addr as i16;
                    continue;
                }

                /*
                 * Binary If Instructions
                 */
                Instruction::BinaryIf(cond, offset) => {
                    let right = *self.ram.get(self.sp as usize).unwrap_or(&0);
                    let left = *self.ram.get((self.sp + 1) as usize).unwrap_or(&0);

                    let taken = match cond {
                        0 => left == right,
                        1 => left != right,
                        2 => left < right,
                        3 => left > right,
                        4 => left <= right,
                        5 => left >= right,
                        _ => false,
                    };
                    if taken {
                        self.pc += offset as i16;
                        continue;
                    }
                }

                /*
                 * Unary If Instructions
                 */
                Instruction::EqZero(offset) => {
                    if self.unary_if(offset, |x| x == 0) {
                        continue;
                    }
                }
                Instruction::NeZero(offset) => {
                    if self.unary_if(offset, |x| x != 0) {
                        continue;
                    }
                }
                Instruction::GeZero(offset) => {
                    if self.unary_if(offset, |x| x >= 0) {
                        continue;
                    }
                }
                Instruction::LtZero(offset) => {
                    if self.unary_if(offset, |x| x < 0) {
                        continue;
                    }
                }

                Instruction::Dup(offset) => {
                    let idx = (self.sp + (offset >> 2) as i16) as usize;
                    let val = self.ram[idx];
                    self.push(val)?;
                }

                Instruction::Print(offset) => {
                    let idx = (self.sp + (offset >> 2) as i16) as usize;
                    let val = self.ram[idx];

                    match offset & 0b11 {
                        0b00 => writeln!(self.output, "{}", val as i32)?,
                        0b01 => writeln!(self.output, "{:#x}", val as i32)?,
                        0b10 => writeln!(self.output, "0b{:b}", val as i32)?,
                        0b11 => writeln!(self.output, "0o{:o}", val as i32)?,
                        _ => unreachable!(),
                    }
                    self.output.flush()?;
                },

                Instruction::Dump() => {
                    if self.sp == 1024 {
                        // stack empty (nop)

                    } else {
                        for offset in self.sp..1024 {
                            let address = offset - self.sp;
                            let value = self.ram[offset as usize];
                            writeln!(self.output, "{:04x}: {:08x}", address, value)?;
                        }

                        self.output.flush()?;
                    }

                }


                Instruction::Push(val) => self.push(val)?, 
            }

            self.step(); 
        }
    }

    /*
     * Binary arithmetic helper function
     */
    fn binary_op<F>(&mut self, op: F)
    where
        F: Fn(i32, i32) -> i32,
        {
            // Get right operand
            let right = self.ram[self.sp as usize] as i32;
            self.sp += 1;

            // Get left operand
            let left = self.ram[self.sp as usize] as i32;
            self.sp += 1;

            // Apply binary operation to operands
            let result = op(left, right);

            self.sp -= 1;
            self.ram[self.sp as usize] = result as u32;
        }

    /*
     * Unary arithmetic helper function
     */
    fn unary_op<F>(&mut self, op: F)
    where
        F: Fn(i32) -> i32,
        {
            let val = self.ram[self.sp as usize] as i32;
            self.sp += 1;

            let result = op(val);
            self.sp -= 1;
            self.ram[self.sp as usize] = result as u32;
        }

    /*
     * Unary If helper function
     */
    fn unary_if<F>(&mut self, offset: i32, cond: F) -> bool
    where
        F: Fn(i32) -> bool,
        {
            let val = self.ram[self.sp as usize] as i32;
            if cond(val) {
                self.pc += (offset >> 2) as i16;
                return true; // Jump occurred
            }
            false
        }

    // Increment program counter
    fn step(&mut self) {
        self.pc += 1;
    }

    // Push a value onto the stack
    fn push(&mut self, word: u32) -> Result<(), Box<dyn std::error::Error>> {
        if self.sp <= 0 {
            return Err("Overflow".into());
        }
        self.sp -= 1;
        self.ram[self.sp as usize] = word;
        Ok(())
    }

    // Decode an instruction from RAM
    fn fetch(&self) -> Instruction {
        let inst = self.ram[self.pc as usize];
        let opcode = Opcode::from_integer(((inst >> 28) & 0xF) as u8);

        use Instruction::*;
        match opcode {
            Opcode::Miscellaneous => match (inst >> 24) & 0xF {
                0x0 => Exit(inst as u8 & 0xFF),
                0x1 => Swap((inst >> 12) as i16 & 0xFFF, inst as i16 & 0xFFF),
                0x2 => Nop(),
                0x4 => Input(),
                0x5 => Stinput(inst & 0xFFFFFF),
                0xF => Debug(inst & 0xFFFFFF),
                _ => panic!("Invalid Miscellaneous Instruction"),
            },

            Opcode::BinaryArithmetic => match (inst >> 24) & 0xF {
                0x0 => Add(),
                0x1 => Subtract(),
                0x2 => Multiply(),
                0x3 => Divide(),
                0x4 => Remainder(),
                0x5 => And(),
                0x6 => Or(),
                0x7 => Xor(),
                0x8 => LogicalLeftShift(),
                0x9 => LogicalRightShift(),
                0xB => ArithmeticRightShift(),
                _ => panic!("Invalid Binary Arithmetic Instruction"),
            },


            Opcode::UnaryArithmetic => match (inst >> 24) & 0xF {
                0x0 => Negate(),
                0x1 => Not(),
                _ => panic!("Invalid Unary Arithmetic Instruction"),
            },

            Opcode::Pop => Pop(inst & 0x0FFF_FFFF),

            Opcode::Goto => {
                // Extract offset
                let raw = (inst >> 2) & 0x03FF_FFFF;

                let offset = if (raw & (1 << 25)) != 0 {
                    // Sign extend negative offset
                    (raw | !0x03FF_FFFF) as i32
                } else {
                    raw as i32
                };
                Goto(offset)
            },

            Opcode::StringPrint => Stprint(inst as i32 & 0x0FFF_FFFF),

            Opcode::Call => {
                let mut offset = (inst & 0x03FF_FFFF) as i32;
                if (offset >> 25) & 1 == 1 {
                    offset |= !0x03FF_FFFF;
                }
                Instruction::Call(offset)
            },
            Opcode::Return => {
                let offset = (inst & 0x03FF_FFFF) as u32;
                Instruction::Return(offset)
            }

            Opcode::BinaryIf => {
                let cond = (inst >> 25) & 0b111;
                let raw = (inst >> 2) & 0x007F_FFFF;
                let offset = if raw & (1 << 22) != 0 {
                    (raw as i32) | !0x007F_FFFF
                } else {
                    raw as i32
                };
                BinaryIf(cond as u8, offset as i32)
            }

            Opcode::UnaryIf => {

                // Branching instruction (e.g. EqZero, NeZero, etc.)
                let func2 = (inst >> 25) & 0b11;
                let offset = {
                    let mut val = inst as i32 & 0x00FF_FFFF;
                    if val >> 23 == 1 {

                        // Sign extend negative values
                        val |= 0xFF00_0000u32 as i32;
                    }
                    val
                };
                match func2 {
                    0b00 => EqZero(offset),
                    0b01 => NeZero(offset),
                    0b10 => LtZero(offset),
                    0b11 => GeZero(offset),
                    _ => unreachable!(),
                }
            }
            Opcode::Dup => {
                let offset = inst & 0x0FFF_FFFF;
                Dup(offset) 
            }

            Opcode::Print => Print(inst as i32 & 0x0FFF_FFFF),
            Opcode::Dump => Dump(),
            Opcode::Push => {

                // Push a signed immediate value
                let mut val = inst & 0x0FFF_FFFF;
                if (val >> 27) == 1 {
                    val |= 0xF000_0000;
                }
                Push(val)
            }

            _ => panic!("Unimplemented opcode"),
        }
    }

    // Read a line of input from stdin
    fn read_line(&mut self) -> Result<String, Box<dyn std::error::Error>> {
        let mut s = String::new();
        let mut buf = [0; 1];

        // Read one byte at a time until newline or null
        while self.input.read(&mut buf).map_err(|_| "IO error")? > 0 {
            let c = buf[0] as char;
            if c == '\n' || c == '\0' {
                break;
            }
            s.push(c);
        }

        Ok(s)
    }
}
