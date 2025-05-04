use std::io::{self, Read, Write};
use std::env::args;
use std::fs;

// Entry point of the program
fn main() {
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
    Exit(u8),        
    Swap(i16, i16),   
    Nop(),           
    Input(),          
    Stinput(u32),    
    Pop(u32),         
    Stprint(i32),     
    EqZero(i32),      
    NeZero(i32),      
    LtZero(i32),      
    GeZero(i32),     
    Push(u32),        
    Debug(u32),       
}

// Top-level instruction class based on opcode nibble
#[derive(Debug)]
enum Opcode {
    Miscellaneous, 
    Pop,          
    StringPrint,   
    UnaryIf,     
    Push,          
    Unknown,       
}

impl Opcode {

    // Convert the top 4 bits (opcode) of an instruction to an `Opcode` enum
    fn from_integer(n: u8) -> Opcode {
        match n {
            0x0 => Opcode::Miscellaneous,
            0x1 => Opcode::Pop,
            0x2 => Opcode::StringPrint,
            0x3 => Opcode::UnaryIf,
            0x4 => Opcode::Push,
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

                    // Swap two words in the stack (from and to are relative to SP)
                    let f = (self.sp + (from >> 2)) as usize;
                    let t = (self.sp + (to >> 2)) as usize;
                    self.ram.swap(f, t);
                }

                Instruction::Nop() => (), 

                Instruction::Input() => {

                    // Read a number (decimal/hex/bin) from user
                    let line = self.read_line()?;
                    let word = if let Some(stripped) = line.trim().strip_prefix("0x") {
                        u32::from_str_radix(stripped, 16)
                    } else if let Some(stripped) = line.trim().strip_prefix("0b") {
                        u32::from_str_radix(stripped, 2)
                    } else {
                        u32::from_str_radix(line.trim(), 10)
                    }
                    .map_err(|_| "Failed to parse input")?;
                    self.push(word)?;
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
                        input.push(1 as u8 as char);
                    }

                    // Encode string backwards into stack
                    for (i, chunk) in input.as_bytes().rchunks(3).enumerate() {
                        let word = ((chunk[0] as u32) << 16)
                            | ((chunk[1] as u32) << 8)
                            | (chunk[2] as u32)
                            | if i != 0 { 0x0100_0000 } else { 0 };

                        self.push(word)?;
                    }
                }

                Instruction::Pop(offset) => {

                    // Pop offset bytes (in 4-byte words) from the stack
                    self.sp = (self.sp + (offset >> 2) as i16).clamp(0, 1024);
                }

                Instruction::Stprint(offset) => {

                    // Print a packed string from RAM starting at offset
                    let mut idx = (self.sp + (offset >> 2) as i16) as usize;
                    loop {
                        let bytes = self.ram[idx].to_be_bytes(); 
                        for &b in &bytes[1..] {
                            if b != 1 {
                                self.output.write_all(&[b])?;
                            }
                        }

                        if bytes[0] == 0 || idx == 0 {
                            break;
                        }
                        idx += 1;
                    }
                    self.output.flush()?;
                }

                Instruction::EqZero(offset) => {

                    // Conditional jump if top of stack == 0
                    if self.ram[self.sp as usize] == 0 {
                        self.pc += (offset >> 2) as i16;
                        continue;
                    }
                    println!("eqz");
                }

                Instruction::Push(val) => {
                    println!("push {}", val);
                    self.push(val)?
                }, 

                _ => return Err("Unimplemented instruction".into()), 
            }

            self.step(); 
        }
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
                0x0 => Exit(inst as u8 & 0xF),
                0x1 => Swap((inst >> 12) as i16 & 0xFFF, inst as i16 & 0xFFF),
                0x2 => Nop(),
                0x4 => Input(),
                0x5 => Stinput(inst & 0xFFFFFF),
                0xF => Debug(inst & 0xFFFFFF),
                f => panic!("Invalid"),
            },

            Opcode::Pop => Pop(inst & 0x0FFF_FFFF),

            Opcode::StringPrint => Stprint(inst as i32 & 0x0FFF_FFFF),

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
